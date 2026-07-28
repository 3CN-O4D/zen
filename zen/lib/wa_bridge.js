const {
  makeWASocket, DisconnectReason, useMultiFileAuthState,
  downloadMediaMessage, fetchLatestBaileysVersion,
  generateWAMessageFromContent, generateWAMessageContent,
  prepareWAMessageMedia, getContentType, extractMessageContent,
  normalizeMessageContent, generateForwardMessageContent,
  areJidsSameUser, jidNormalizedUser, isJidGroup, isJidUser,
  proto, Browsers,
} = require('baileys');
const fs = require('fs');
const path = require('path');
const dns = require('dns');
dns.setDefaultResultOrder('ipv4first');
const { tmpdir } = require('os');
const qrcode = require('qrcode-terminal');
const pino = require('pino');
const silentLogger = pino({ level: 'silent' });

let sock = null;
let saveCreds = null;
let lastQR = null;
let connectionState = 'idle';

function send(type, data) {
  process.stdout.write(JSON.stringify({ type, ...data }) + '\n');
}

function sendOk(id, result) {
  send('response', { id, result: result ?? 'ok' });
}

function sendError(id, msg) {
  send('response', { id, error: String(msg) });
}

function readFile(p) {
  return fs.readFileSync(path.resolve(String(p)));
}

function mimeFromExt(ext) {
  const m = { jpg: 'image/jpeg', jpeg: 'image/jpeg', png: 'image/png',
    gif: 'image/gif', webp: 'image/webp', mp4: 'video/mp4',
    mkv: 'video/x-matroska', mov: 'video/quicktime', avi: 'video/x-msvideo',
    mp3: 'audio/mpeg', ogg: 'audio/ogg', wav: 'audio/wav',
    m4a: 'audio/mp4', aac: 'audio/aac', pdf: 'application/pdf',
    doc: 'application/msword', docx: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
    xls: 'application/vnd.ms-excel', xlsx: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
    zip: 'application/zip', rar: 'application/x-rar-compressed',
    '7z': 'application/x-7z-compressed', txt: 'text/plain' };
  return m[ext] || 'application/octet-stream';
}

async function handleCommand(cmd) {
  try {
    if (!sock && cmd.method !== 'connect') {
      sendError(cmd.id, 'Not connected. Call connect first.');
      return;
    }
    const { method, args } = cmd;
    const a = args || [];

    switch (method) {

      // ─────────────── Connection ───────────────
      case 'connect':
        const authDir = String(a[0]?.authDir || 'wa_auth');
        const pairingPhone = String(a[0]?.pairingPhone || '');
        const { state, saveCreds: sc } = await useMultiFileAuthState(authDir);
        saveCreds = sc;
        let waVersion;
        try {
          const v = await fetchLatestBaileysVersion();
          waVersion = v.version;
        } catch (e) {
          waVersion = [2, 2413, 54];
        }
        sock = makeWASocket({
          auth: state,
          version: waVersion,
          browser: Browsers.macOS('Desktop'),
          syncFullHistory: false,
          connectTimeoutMs: 60000,
          defaultQueryTimeoutMs: 60000,
          markOnlineOnConnect: false,
          printQRInTerminal: false,
          logger: silentLogger,
        });
        connectionState = 'connecting';
        sock.ev.on('creds.update', saveCreds);
        sock.ev.on('connection.update', ({ connection, lastDisconnect, qr }) => {
          if (qr) {
            lastQR = qr;
            connectionState = 'awaiting_qr';
            send('event', { name: 'qr', data: { qr } });
            qrcode.generate(qr, { small: true }, function(code) {
              send('event', { name: 'qr_display', data: { text: code } });
            });
          }
          if (connection === 'open') {
            connectionState = 'connected';
            send('event', { name: 'connected', data: { jid: sock?.user?.id } });
          }
          if (connection === 'close') {
            const reason = lastDisconnect?.error?.output?.statusCode || DisconnectReason.loggedOut;
            const errorMsg = lastDisconnect?.error?.message || '';
            connectionState = 'disconnected';
            send('event', { name: 'disconnected', data: { reason, error: errorMsg } });
            const reasonName = Object.entries(DisconnectReason).find(([,v]) => v === reason)?.[0] || 'UNKNOWN';
            process.stderr.write('\n[WA] Disconnected: ' + reasonName + ' (' + reason + ')' + (errorMsg ? ' - ' + errorMsg : '') + '\n');
          }
        });
        sock.ev.on('messages.upsert', ({ messages }) => {
          for (const msg of messages) {
            if (msg.key && msg.key.fromMe) continue;
            const content = extractMessageContent(msg.message);
            send('event', {
              name: 'message', data: {
                from: msg.key.remoteJid, id: msg.key.id,
                text: content?.conversation || content?.extendedTextMessage?.text || content?.imageMessage?.caption || content?.videoMessage?.caption || '',
                timestamp: msg.messageTimestamp, sender: msg.key.participant || msg.key.remoteJid,
                type: getContentType(msg.message) || 'unknown',
              }
            });
          }
        });
        sock.ev.on('presence.update', ({ id, presences }) => {
          for (const [jid, presence] of Object.entries(presences)) {
            send('event', { name: 'presence', data: { jid, status: presence.lastKnownPresence } });
          }
        });
        if (pairingPhone) {
          // Pairing code flow: request code immediately during registration
          try {
            const code = await sock.requestPairingCode(pairingPhone);
            sendOk(cmd.id, { status: 'pairing', pairingCode: code });
          } catch (e) {
            sendError(cmd.id, 'Pairing code failed: ' + (e.message || e));
          }
        } else {
          sendOk(cmd.id, { status: 'connecting' });
        }
        break;

      case 'getLastQR':
        sendOk(cmd.id, lastQR || null);
        break;

      case 'clearLastQR':
        lastQR = null;
        sendOk(cmd.id);
        break;

      case 'requestPairingCode': {
        const phone = String(a[0]);
        if (connectionState === 'disconnected' || !sock) {
          sendError(cmd.id, 'WhatsApp connection failed. Check your network and try again. Disconnected.');
          break;
        }
        try {
          const code = await sock.requestPairingCode(phone);
          sendOk(cmd.id, code);
        } catch (e) {
          sendError(cmd.id, 'Pairing code failed: ' + (e.message || e));
        }
        break;
      }

      case 'connectionState':
        sendOk(cmd.id, connectionState);
        break;

      case 'exportAuth': {
        const password = String(a[0] || '');
        const crypto = require('crypto');
        const authDir = String(a[1] || 'wa_auth');
        const files = {};
        const dir = path.resolve(authDir);
        if (fs.existsSync(dir)) {
          for (const f of fs.readdirSync(dir)) {
            const fp = path.join(dir, f);
            if (fs.statSync(fp).isFile()) files[f] = fs.readFileSync(fp).toString('base64');
          }
        }
        const json = JSON.stringify(files);
        if (!password) { sendOk(cmd.id, Buffer.from(json).toString('base64')); break; }
        const key = crypto.createHash('sha256').update(String(password)).digest();
        const iv = crypto.randomBytes(16);
        const cipher = crypto.createCipheriv('aes-256-cbc', key, iv);
        const encrypted = Buffer.concat([cipher.update(json, 'utf8'), cipher.final()]);
        sendOk(cmd.id, iv.toString('base64') + ':' + encrypted.toString('base64'));
        break;
      }

      case 'importAuth': {
        const password = String(a[0] || '');
        const authDir = String(a[1] || 'wa_auth');
        const payload = String(a[2] || '');
        let json;
        if (!password) {
          json = Buffer.from(payload, 'base64').toString('utf8');
        } else {
          const crypto = require('crypto');
          const parts = payload.split(':');
          if (parts.length !== 2) { sendError(cmd.id, 'Invalid encrypted payload'); break; }
          const iv = Buffer.from(parts[0], 'base64');
          const encrypted = Buffer.from(parts[1], 'base64');
          const key = crypto.createHash('sha256').update(String(password)).digest();
          const decipher = crypto.createDecipheriv('aes-256-cbc', key, iv);
          json = decipher.update(encrypted) + decipher.final('utf8');
        }
        const files = JSON.parse(json);
        const dir = path.resolve(authDir);
        if (!fs.existsSync(dir)) fs.mkdirSync(dir, { recursive: true });
        for (const [name, data] of Object.entries(files)) {
          fs.writeFileSync(path.join(dir, name), Buffer.from(data, 'base64'));
        }
        sendOk(cmd.id, Object.keys(files).join(','));
        break;
      }

      case 'disconnect':
        sock?.end();
        sock = null;
        sendOk(cmd.id);
        break;

      case 'logout':
        await sock?.logout();
        sendOk(cmd.id);
        break;

      // ─────────────── Messaging ───────────────
      case 'sendMessage': {
        const [jid, text] = a;
        await sock.sendMessage(String(jid), { text: String(text) });
        sendOk(cmd.id, 'sent');
        break;
      }

      case 'sendImage': {
        const [jid, imgPath, caption] = a;
        const data = readFile(imgPath);
        await sock.sendMessage(String(jid), { image: data, caption: String(caption || '') });
        sendOk(cmd.id, 'sent');
        break;
      }

      case 'sendVideo': {
        const [jid, vidPath, caption] = a;
        const data = readFile(vidPath);
        await sock.sendMessage(String(jid), { video: data, caption: String(caption || '') });
        sendOk(cmd.id, 'sent');
        break;
      }

      case 'sendAudio': {
        const [jid, audioPath, asVoice] = a;
        const data = readFile(audioPath);
        const ext = path.extname(String(audioPath)).slice(1);
        await sock.sendMessage(String(jid), {
          audio: data, mimetype: mimeFromExt(ext) || 'audio/mp4', ptt: asVoice !== false,
        });
        sendOk(cmd.id, 'sent');
        break;
      }

      case 'sendSticker': {
        const [jid, stickerPath] = a;
        const data = readFile(stickerPath);
        await sock.sendMessage(String(jid), { sticker: data });
        sendOk(cmd.id, 'sent');
        break;
      }

      case 'sendDocument': {
        const [jid, docPath, fileName] = a;
        const data = readFile(docPath);
        await sock.sendMessage(String(jid), {
          document: data, fileName: String(fileName || path.basename(String(docPath))),
        });
        sendOk(cmd.id, 'sent');
        break;
      }

      case 'sendViewOnce': {
        const [jid, mediaPath, type, caption] = a;
        const data = readFile(mediaPath);
        const msg = type === 'video' ? { video: data, caption: String(caption || ''), viewOnce: true }
          : { image: data, caption: String(caption || ''), viewOnce: true };
        await sock.sendMessage(String(jid), msg);
        sendOk(cmd.id, 'sent');
        break;
      }

      case 'sendContact': {
        const [jid, displayName, vcard] = a;
        await sock.sendMessage(String(jid), {
          contacts: { displayName: String(displayName), contacts: [{ vcard: String(vcard) }] },
        });
        sendOk(cmd.id, 'sent');
        break;
      }

      case 'sendLocation': {
        const [jid, lat, lon, name] = a;
        await sock.sendMessage(String(jid), {
          location: { degreesLatitude: Number(lat), degreesLongitude: Number(lon), name: String(name || '') },
        });
        sendOk(cmd.id, 'sent');
        break;
      }

      case 'sendPoll': {
        const [jid, question, options] = a;
        await sock.sendMessage(String(jid), {
          poll: { name: String(question), values: (options || []).map(String) },
        });
        sendOk(cmd.id, 'sent');
        break;
      }

      case 'sendReaction': {
        const [jid, msgId, emoji, fromMe, participant] = a;
        const key = { remoteJid: String(jid), id: String(msgId), fromMe: !!fromMe };
        if (participant) key.participant = String(participant);
        await sock.sendMessage(String(jid), { react: { key, text: String(emoji) } });
        sendOk(cmd.id, 'sent');
        break;
      }

      case 'sendReply': {
        const [jid, text, replyMsgId, replyFromMe, replyParticipant] = a;
        const quoted = { remoteJid: String(jid), id: String(replyMsgId), fromMe: !!replyFromMe };
        if (replyParticipant) quoted.participant = String(replyParticipant);
        await sock.sendMessage(String(jid), { text: String(text) }, { quoted });
        sendOk(cmd.id, 'sent');
        break;
      }

      case 'forwardMessage': {
        const [jid, fromJid, msgId, fromMe, participant] = a;
        const key = { remoteJid: String(fromJid), id: String(msgId), fromMe: !!fromMe };
        if (participant) key.participant = String(participant);
        const msgs = await sock.loadMessages(fromJid, 1, undefined, key.id);
        if (!msgs.length) { sendError(cmd.id, 'Message not found'); break; }
        const fwd = await generateForwardMessageContent(msgs[0].message);
        await sock.sendMessage(String(jid), { forward: fwd });
        sendOk(cmd.id, 'sent');
        break;
      }

      case 'deleteMessage': {
        const [jid, msgId, fromMe, participant] = a;
        const key = { remoteJid: String(jid), id: String(msgId), fromMe: !!fromMe };
        if (participant) key.participant = String(participant);
        await sock.sendMessage(String(jid), { delete: key });
        sendOk(cmd.id);
        break;
      }

      case 'starMessage': {
        const [jid, msgId, starred, fromMe, participant] = a;
        const key = { remoteJid: String(jid), id: String(msgId), fromMe: !!fromMe };
        if (participant) key.participant = String(participant);
        await sock.star(key, !!starred);
        sendOk(cmd.id);
        break;
      }

      // ─────────────── Media ───────────────
      case 'downloadMedia': {
        const [jid, msgId, fromMe, participant] = a;
        const key = { remoteJid: String(jid), id: String(msgId), fromMe: !!fromMe };
        if (participant) key.participant = String(participant);
        const msgs = await sock.loadMessages(jid, 1, undefined, msgId);
        if (!msgs.length) { sendError(cmd.id, 'Message not found'); break; }
        const content = normalizeMessageContent(msgs[0].message);
        const stream = await downloadMediaMessage(key, 'buffer', {}, { reuploadRequest: sock.updateMediaMessage });
        const fileName = `${msgId}.${(getContentType(msgs[0].message) || '').replace(/Message$/, '')}`;
        const outPath = path.join(tmpdir(), fileName);
        fs.writeFileSync(outPath, stream);
        sendOk(cmd.id, outPath);
        break;
      }

      // ─────────────── Groups ───────────────
      case 'groupCreate': {
        const result = await sock.groupCreate(String(a[0]), (a[1] || []).map(String));
        sendOk(cmd.id, result.id);
        break;
      }

      case 'groupAdd':
        await sock.groupParticipantsUpdate(String(a[0]), [String(a[1])], 'add');
        sendOk(cmd.id);
        break;

      case 'groupRemove':
        await sock.groupParticipantsUpdate(String(a[0]), [String(a[1])], 'remove');
        sendOk(cmd.id);
        break;

      case 'groupPromote':
        await sock.groupParticipantsUpdate(String(a[0]), [String(a[1])], 'promote');
        sendOk(cmd.id);
        break;

      case 'groupDemote':
        await sock.groupParticipantsUpdate(String(a[0]), [String(a[1])], 'demote');
        sendOk(cmd.id);
        break;

      case 'groupLeave':
        await sock.groupLeave(String(a[0]));
        sendOk(cmd.id);
        break;

      case 'groupMetadata': {
        const meta = await sock.groupMetadata(String(a[0]));
        sendOk(cmd.id, meta);
        break;
      }

      case 'groupInviteCode': {
        const code = await sock.groupInviteCode(String(a[0]));
        sendOk(cmd.id, code);
        break;
      }

      case 'groupRevokeInvite':
        await sock.groupRevokeInvite(String(a[0]));
        sendOk(cmd.id);
        break;

      case 'groupAcceptInvite': {
        const gid = await sock.groupAcceptInvite(String(a[0]));
        sendOk(cmd.id, gid);
        break;
      }

      case 'groupUpdateSubject':
        await sock.groupUpdateSubject(String(a[0]), String(a[1]));
        sendOk(cmd.id);
        break;

      case 'groupUpdateDescription':
        await sock.groupUpdateDescription(String(a[0]), String(a[1]));
        sendOk(cmd.id);
        break;

      case 'groupSettingUpdate':
        await sock.groupSettingUpdate(String(a[0]), String(a[1]));
        sendOk(cmd.id);
        break;

      case 'groupJoinApproval':
        await sock.groupJoinApprovalMode(String(a[0]), !!a[1]);
        sendOk(cmd.id);
        break;

      case 'groupToggleEphemeral':
        await sock.groupToggleEphemeral(String(a[0]), Number(a[1]) || 0);
        sendOk(cmd.id);
        break;

      case 'groupMemberAddMode':
        await sock.groupMemberAddMode(String(a[0]), String(a[1]));
        sendOk(cmd.id);
        break;

      case 'groupRequestParticipantsList': {
        const list = await sock.groupRequestParticipantsList(String(a[0]));
        sendOk(cmd.id, list);
        break;
      }

      case 'groupRequestParticipantsUpdate':
        await sock.groupRequestParticipantsUpdate(String(a[0]), (a[1] || []).map(String), String(a[2]));
        sendOk(cmd.id);
        break;

      case 'groupFetchAllParticipating': {
        const groups = await sock.groupFetchAllParticipating();
        sendOk(cmd.id, Object.keys(groups));
        break;
      }

      case 'groupGetInviteInfo': {
        const info = await sock.groupGetInviteInfo(String(a[0]));
        sendOk(cmd.id, info);
        break;
      }

      // ─────────────── Status / Stories ───────────────
      case 'sendStatusText': {
        await sock.sendMessage('status@broadcast', { text: String(a[0]) });
        sendOk(cmd.id);
        break;
      }

      case 'sendStatusImage': {
        const data = readFile(String(a[0]));
        await sock.sendMessage('status@broadcast', { image: data, caption: String(a[1] || '') });
        sendOk(cmd.id);
        break;
      }

      case 'sendStatusVideo': {
        const data = readFile(String(a[0]));
        await sock.sendMessage('status@broadcast', { video: data, caption: String(a[1] || '') });
        sendOk(cmd.id);
        break;
      }

      case 'fetchStatus': {
        const statuses = [];
        for (const jid of a) {
          try { statuses.push({ jid, status: await sock.fetchStatus(String(jid)) }); }
          catch (e) { statuses.push({ jid, error: e.message }); }
        }
        sendOk(cmd.id, statuses);
        break;
      }

      // ─────────────── Contacts ───────────────
      case 'getContacts': {
        const list = Object.entries(sock.contacts || {}).map(([jid, c]) => ({
          jid, name: c.name || c.notify || '', pushname: c.notify || '',
        }));
        sendOk(cmd.id, list);
        break;
      }

      case 'updateBlock':
        await sock.updateBlockStatus(String(a[0]), String(a[1]));
        sendOk(cmd.id);
        break;

      case 'fetchBlocklist': {
        const list = await sock.fetchBlocklist();
        sendOk(cmd.id, list);
        break;
      }

      case 'onWhatsApp': {
        const results = [];
        for (const id of a) {
          try { results.push(await sock.onWhatsApp(String(id))); }
          catch (e) { results.push({ jid: String(id), exists: false }); }
        }
        sendOk(cmd.id, results);
        break;
      }

      case 'addContact':
        await sock.addOrEditContact(String(a[0]), String(a[1]));
        sendOk(cmd.id);
        break;

      case 'removeContact':
        await sock.removeContact(String(a[0]));
        sendOk(cmd.id);
        break;

      // ─────────────── Profile ───────────────
      case 'updateProfileName':
        await sock.updateProfileName(String(a[0]));
        sendOk(cmd.id);
        break;

      case 'updateProfileStatus':
        await sock.updateProfileStatus(String(a[0]));
        sendOk(cmd.id);
        break;

      case 'updateProfilePicture': {
        const data = readFile(String(a[1]));
        await sock.updateProfilePicture(String(a[0]), data);
        sendOk(cmd.id);
        break;
      }

      case 'removeProfilePicture':
        await sock.removeProfilePicture(String(a[0]));
        sendOk(cmd.id);
        break;

      case 'profilePictureUrl': {
        const url = await sock.profilePictureUrl(String(a[0]), a[1] || 'image');
        sendOk(cmd.id, url);
        break;
      }

      // ─────────────── Chat Operations ───────────────
      case 'chatModify': {
        const [jid, action, value] = a;
        await sock.chatModify({ [String(action)]: value ?? true }, String(jid));
        sendOk(cmd.id);
        break;
      }

      case 'readMessages': {
        const [jid, msgId, participant] = a;
        const key = { remoteJid: String(jid), id: String(msgId), fromMe: true };
        if (participant) key.participant = String(participant);
        await sock.readMessages([key]);
        sendOk(cmd.id);
        break;
      }

      // ─────────────── Presence ───────────────
      case 'sendPresenceUpdate':
        await sock.sendPresenceUpdate(String(a[0] || 'available'));
        sendOk(cmd.id);
        break;

      case 'typingPresence':
        await sock.sendPresenceUpdate('composing', String(a[0]));
        sendOk(cmd.id);
        break;

      case 'recordingPresence':
        await sock.sendPresenceUpdate('recording', String(a[0]));
        sendOk(cmd.id);
        break;

      // ─────────────── Privacy ───────────────
      case 'fetchPrivacySettings': {
        const settings = await sock.fetchPrivacySettings();
        sendOk(cmd.id, settings);
        break;
      }

      case 'updateLastSeenPrivacy':
        await sock.updateLastSeenPrivacy(String(a[0]));
        sendOk(cmd.id);
        break;

      case 'updateReadReceiptsPrivacy':
        await sock.updateReadReceiptsPrivacy(String(a[0]));
        sendOk(cmd.id);
        break;

      case 'updateOnlinePrivacy':
        await sock.updateOnlinePrivacy(String(a[0]));
        sendOk(cmd.id);
        break;

      case 'updateProfilePicturePrivacy':
        await sock.updateProfilePicturePrivacy(String(a[0]));
        sendOk(cmd.id);
        break;

      case 'updateStatusPrivacy':
        await sock.updateStatusPrivacy(String(a[0]));
        sendOk(cmd.id);
        break;

      case 'updateGroupsAddPrivacy':
        await sock.updateGroupsAddPrivacy(String(a[0]));
        sendOk(cmd.id);
        break;

      case 'updateDisableLinkPreviewsPrivacy':
        await sock.updateDisableLinkPreviewsPrivacy(!!a[0]);
        sendOk(cmd.id);
        break;

      case 'updateMessagesPrivacy':
        await sock.updateMessagesPrivacy(String(a[0]));
        sendOk(cmd.id);
        break;

      case 'updateCallPrivacy':
        await sock.updateCallPrivacy(String(a[0]));
        sendOk(cmd.id);
        break;

      // ─────────────── Newsletters ───────────────
      case 'loadMessages': {
        const [jid, count, cursor, msgId] = a;
        const msgs = await sock.loadMessages(String(jid), Number(count) || 10, cursor || undefined, String(msgId || '') || undefined);
        sendOk(cmd.id, msgs);
        break;
      }

      case 'newsletterCreate': {
        const result = await sock.newsletterCreate(String(a[0]), String(a[1] || ''), a[2] ? readFile(String(a[2])) : undefined);
        sendOk(cmd.id, result);
        break;
      }

      case 'newsletterFollow':
        await sock.newsletterFollow(String(a[0]));
        sendOk(cmd.id);
        break;

      case 'newsletterUnfollow':
        await sock.newsletterUnfollow(String(a[0]));
        sendOk(cmd.id);
        break;

      case 'newsletterMetadata': {
        const meta = await sock.newsletterMetadata(String(a[0]));
        sendOk(cmd.id, meta);
        break;
      }

      case 'newsletterFetchMessages': {
        const msgs = await sock.newsletterFetchMessages(String(a[0]), Number(a[1]) || 10);
        sendOk(cmd.id, msgs);
        break;
      }

      case 'newsletterUpdateName':
        await sock.newsletterUpdateName(String(a[0]), String(a[1]));
        sendOk(cmd.id);
        break;

      case 'newsletterUpdateDescription':
        await sock.newsletterUpdateDescription(String(a[0]), String(a[1]));
        sendOk(cmd.id);
        break;

      case 'newsletterUpdatePicture': {
        const data = readFile(String(a[1]));
        await sock.newsletterUpdatePicture(String(a[0]), data);
        sendOk(cmd.id);
        break;
      }

      case 'newsletterRemovePicture':
        await sock.newsletterRemovePicture(String(a[0]));
        sendOk(cmd.id);
        break;

      case 'newsletterDelete':
        await sock.newsletterDelete(String(a[0]));
        sendOk(cmd.id);
        break;

      case 'newsletterMute':
        await sock.newsletterMute(String(a[0]));
        sendOk(cmd.id);
        break;

      case 'newsletterUnmute':
        await sock.newsletterUnmute(String(a[0]));
        sendOk(cmd.id);
        break;

      case 'newsletterSubscribers': {
        const subs = await sock.newsletterSubscribers(String(a[0]));
        sendOk(cmd.id, subs);
        break;
      }

      case 'newsletterAdminCount': {
        const count = await sock.newsletterAdminCount(String(a[0]));
        sendOk(cmd.id, count);
        break;
      }

      case 'newsletterChangeOwner':
        await sock.newsletterChangeOwner(String(a[0]), String(a[1]));
        sendOk(cmd.id);
        break;

      case 'newsletterDemote':
        await sock.newsletterDemote(String(a[0]), String(a[1]));
        sendOk(cmd.id);
        break;

      case 'newsletterReactMessage':
        await sock.newsletterReactMessage(String(a[0]), String(a[1]), String(a[2]));
        sendOk(cmd.id);
        break;

      case 'newsletterUpdate':
        await sock.newsletterUpdate(String(a[0]), a[1]);
        sendOk(cmd.id);
        break;

      case 'subscribeNewsletterUpdates':
        await sock.subscribeNewsletterUpdates((a[0] || []).map(String));
        sendOk(cmd.id);
        break;

      // ─────────────── User Info ───────────────
      case 'getUser':
        sendOk(cmd.id, sock.user || null);
        break;

      case 'getBusinessProfile': {
        const bp = await sock.getBusinessProfile(String(a[0]));
        sendOk(cmd.id, bp);
        break;
      }

      default:
        sendError(cmd.id, `Unknown method: ${method}`);
    }
  } catch (e) {
    sendError(cmd.id, e.message);
  }
}

process.stdin.setEncoding('utf8');
let buffer = '';
process.stdin.on('data', (chunk) => {
  buffer += chunk;
  const lines = buffer.split('\n');
  buffer = lines.pop();
  for (const line of lines) {
    if (!line.trim()) continue;
    try { handleCommand(JSON.parse(line)); }
    catch (e) { send('error', { message: 'Invalid JSON: ' + e.message }); }
  }
});
process.stdin.on('end', () => { sock?.end(); process.exit(0); });
