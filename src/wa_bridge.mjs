// zen-wa-bridge: Node/Baileys v7 (ESM) subprocess driven by Zen's `wa` module.
// Protocol (newline-delimited JSON on stdout/stdin):
//   bridge -> host: {"type":"state","state":..., "qr"?, "code"?, "reason"?}
//                   {"type":"message", id, from, from_alt, sender, sender_alt,
//                    push_name, text, is_group, timestamp}
//                   {"type":"result","id":n,"ok":bool,"error"?}
//   host -> bridge: {"op":"send_text","id":n,"to":jid,"text":s}
//                   {"op":"logout","id":n}
//                   {"op":"shutdown"}

'use strict';
import fs from 'node:fs';
import readline from 'node:readline';
import pino from 'pino';
import qrcodeTerminal from 'qrcode-terminal';
import makeWASocket, {
    useMultiFileAuthState,
    DisconnectReason,
    fetchLatestBaileysVersion,
} from 'baileys';

const logger = pino({ level: 'silent' });

function out(obj) {
    process.stdout.write(JSON.stringify(obj) + '\n');
}

// ── args ──
const argv = process.argv.slice(2);
function argOf(name) {
    const i = argv.indexOf(name);
    return i >= 0 ? argv[i + 1] : undefined;
}
const authDir = argOf('--auth') || 'wa_auth';
const phone = argOf('--phone');

let sock = null;
let shuttingDown = false;
let pairingRequested = false;

function extractText(m) {
    const msg = m.message;
    if (!msg) return '';
    return msg.conversation ||
        msg.extendedTextMessage?.text ||
        msg.imageMessage?.caption ||
        msg.videoMessage?.caption || '';
}

async function startSock() {
    if (shuttingDown) return;
    const { state, saveCreds } = await useMultiFileAuthState(authDir);
    let version;
    try {
        const v = await fetchLatestBaileysVersion();
        version = v.version;
    } catch { /* offline: bundled proto */ }

    sock = makeWASocket({
        version,
        auth: state,
        logger,
        printQRInTerminal: false,
        syncFullHistory: false,
        markOnlineOnConnect: true,
    });

    sock.ev.on('creds.update', saveCreds);

    sock.ev.on('connection.update', async (u) => {
        const { connection, lastDisconnect, qr } = u;
        if (qr) {
            // Pairing-code mode: trade the QR for an 8-char code.
            if (phone && !pairingRequested) {
                pairingRequested = true;
                try {
                    const code = await sock.requestPairingCode(phone);
                    out({ type: 'state', state: 'pairing', code });
                } catch (e) {
                    out({ type: 'state', state: 'dead', reason: 'pairing code failed: ' + e });
                }
                return;
            }
            // Render for the human on stderr; keep stdout pure JSON.
            qrcodeTerminal.generate(qr, { small: true }, (art) => {
                process.stderr.write(art + '\n');
            });
            out({ type: 'state', state: 'qr', qr });
            return;
        }
        if (connection === 'connecting') {
            out({ type: 'state', state: 'connecting' });
        } else if (connection === 'open') {
            out({ type: 'state', state: 'connected' });
        } else if (connection === 'close') {
            if (shuttingDown) return;
            const code = lastDisconnect?.error?.output?.statusCode;
            if (code === DisconnectReason.loggedOut) {
                out({ type: 'state', state: 'dead', reason: 'logged_out' });
                process.exit(0);
            }
            out({ type: 'state', state: 'disconnected', reason: String(code ?? '?') });
            setTimeout(() => {
                startSock().catch((e) => out({ type: 'state', state: 'dead', reason: String(e) }));
            }, 3000);
        }
    });

    sock.ev.on('messages.upsert', ({ messages }) => {
        for (const m of messages) {
            if (!m.key || !m.message) continue;
            if (m.key.fromMe) continue; // never react to our own sends
            const jid = m.key.remoteJid;
            if (!jid || jid === 'status@broadcast') continue;
            const text = extractText(m);
            if (!text) continue;
            out({
                type: 'message',
                id: String(m.key.id ?? ''),
                from: jid,
                from_alt: String(m.key.remoteJidAlt || ''),
                sender: String(m.key.participant || jid),
                sender_alt: String(m.key.participantAlt || ''),
                push_name: String(m.pushName || ''),
                text,
                is_group: !!m.key.participant,
                timestamp: Number(m.messageTimestamp || 0),
            });
        }
    });
}

startSock().catch((e) => {
    out({ type: 'state', state: 'dead', reason: String(e && e.message || e) });
    process.exit(1);
});

// ── host commands ──
const rl = readline.createInterface({ input: process.stdin, terminal: false });
rl.on('line', (line) => {
    let o;
    try { o = JSON.parse(line); } catch { return; }
    if (!o || !o.op) return;
    if (o.op === 'send_text') {
        if (!sock) { out({ type: 'result', id: o.id, ok: false, error: 'socket not ready' }); return; }
        sock.sendMessage(o.to, { text: o.text })
            .then(() => out({ type: 'result', id: o.id, ok: true }))
            .catch((e) => out({ type: 'result', id: o.id, ok: false, error: String(e && e.message || e) }));
    } else if (o.op === 'logout') {
        shuttingDown = true;
        const finish = () => {
            try { fs.rmSync(authDir, { recursive: true, force: true }); } catch { }
            out({ type: 'result', id: o.id, ok: true });
            process.exit(0);
        };
        if (sock) {
            sock.logout().then(finish).catch(finish);
            setTimeout(finish, 5000).unref();
        } else finish();
    } else if (o.op === 'shutdown') {
        shuttingDown = true;
        out({ type: 'result', id: o.id, ok: true });
        process.exit(0);
    }
});

process.on('SIGTERM', () => process.exit(0));
