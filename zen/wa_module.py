import subprocess
import threading
import json
import os
import sys
from .environment import ZenError

_BRIDGE_SCRIPT = os.path.join(os.path.dirname(__file__), 'lib', 'wa_bridge.js')


class _WAResult:
    def __init__(self):
        self.value = None
        self.error = None
        self._event = threading.Event()

    def set(self, value):
        self.value = value
        self._event.set()

    def set_error(self, error):
        self.error = error
        self._event.set()

    def wait(self, timeout=60):
        self._event.wait(timeout=timeout)
        if self.error:
            raise ZenError(self.error)
        return self.value


class _WAConnection:
    def __init__(self, proc, reader_thread):
        self._proc = proc
        self._reader = reader_thread
        self._pending = {}
        self._cmd_id = 0
        self._lock = threading.Lock()
        self._callbacks = {}
        self._last_qr = None

    def _send_cmd(self, method, args=None, timeout=60):
        if args is None:
            args = []
        with self._lock:
            self._cmd_id += 1
            cmd_id = self._cmd_id
            cmd = json.dumps({'type': 'command', 'id': cmd_id, 'method': method, 'args': args})
            result = _WAResult()
            self._pending[cmd_id] = result
            self._proc.stdin.write((cmd + '\n').encode('utf-8'))
            self._proc.stdin.flush()
        return result.wait(timeout)

    # ── Connection ──
    def connect(self, auth_dir='wa_auth', pairing_phone=None):
        args = {'authDir': auth_dir}
        if pairing_phone:
            args['pairingPhone'] = pairing_phone
        result = self._send_cmd('connect', [args], timeout=90)
        self.pairingCode = result.get('pairingCode') if isinstance(result, dict) else None
        return result

    def request_pairing_code(self, phone):
        return self._send_cmd('requestPairingCode', [phone], timeout=30)

    def connection_state(self):
        try:
            return self._send_cmd('connectionState')
        except Exception:
            return 'dead'

    def export_auth(self, password='', auth_dir='wa_auth'):
        return self._send_cmd('exportAuth', [password, auth_dir])

    def import_auth(self, password='', auth_dir='wa_auth', payload=''):
        return self._send_cmd('importAuth', [password, auth_dir, payload])

    def disconnect(self):
        try:
            self._send_cmd('disconnect', timeout=5)
        except Exception:
            pass
        try:
            self._proc.kill()
        except Exception:
            pass

    def logout(self):
        return self._send_cmd('logout')

    # ── Messaging ──
    def send(self, jid, text):
        return self._send_cmd('sendMessage', [jid, text])

    def send_image(self, jid, path, caption=''):
        return self._send_cmd('sendImage', [jid, path, caption])

    def send_video(self, jid, path, caption=''):
        return self._send_cmd('sendVideo', [jid, path, caption])

    def send_audio(self, jid, path, as_voice=True):
        return self._send_cmd('sendAudio', [jid, path, as_voice])

    def send_sticker(self, jid, path):
        return self._send_cmd('sendSticker', [jid, path])

    def send_document(self, jid, path, filename=None):
        return self._send_cmd('sendDocument', [jid, path, filename or ''])

    def send_view_once(self, jid, path, media_type='image', caption=''):
        return self._send_cmd('sendViewOnce', [jid, path, media_type, caption])

    def send_contact(self, jid, display_name, vcard):
        return self._send_cmd('sendContact', [jid, display_name, vcard])

    def send_location(self, jid, lat, lon, name=''):
        return self._send_cmd('sendLocation', [jid, lat, lon, name])

    def send_poll(self, jid, question, options):
        return self._send_cmd('sendPoll', [jid, question, list(options)])

    def send_reaction(self, jid, msg_id, emoji, from_me=False, participant=None):
        return self._send_cmd('sendReaction', [jid, msg_id, emoji, from_me, participant or ''])

    def send_reply(self, jid, text, reply_msg_id, reply_from_me=False, reply_participant=None):
        return self._send_cmd('sendReply', [jid, text, reply_msg_id, reply_from_me, reply_participant or ''])

    def forward_message(self, jid, from_jid, msg_id, from_me=False, participant=None):
        return self._send_cmd('forwardMessage', [jid, from_jid, msg_id, from_me, participant or ''])

    def delete_message(self, jid, msg_id, from_me=False, participant=None):
        return self._send_cmd('deleteMessage', [jid, msg_id, from_me, participant or ''])

    def star_message(self, jid, msg_id, starred=True, from_me=False, participant=None):
        return self._send_cmd('starMessage', [jid, msg_id, starred, from_me, participant or ''])

    # ── Media ──
    def download_media(self, jid, msg_id, from_me=False, participant=None):
        return self._send_cmd('downloadMedia', [jid, msg_id, from_me, participant or ''])

    # ── Groups ──
    def group_create(self, name, participants):
        return self._send_cmd('groupCreate', [name, list(participants)])

    def group_add(self, group_jid, participant):
        return self._send_cmd('groupAdd', [group_jid, participant])

    def group_remove(self, group_jid, participant):
        return self._send_cmd('groupRemove', [group_jid, participant])

    def group_promote(self, group_jid, participant):
        return self._send_cmd('groupPromote', [group_jid, participant])

    def group_demote(self, group_jid, participant):
        return self._send_cmd('groupDemote', [group_jid, participant])

    def group_leave(self, group_jid):
        return self._send_cmd('groupLeave', [group_jid])

    def group_info(self, group_jid):
        return self._send_cmd('groupMetadata', [group_jid])

    def group_invite_code(self, group_jid):
        return self._send_cmd('groupInviteCode', [group_jid])

    def group_revoke_invite(self, group_jid):
        return self._send_cmd('groupRevokeInvite', [group_jid])

    def group_accept_invite(self, code):
        return self._send_cmd('groupAcceptInvite', [code])

    def group_set_subject(self, group_jid, subject):
        return self._send_cmd('groupUpdateSubject', [group_jid, subject])

    def group_set_description(self, group_jid, description):
        return self._send_cmd('groupUpdateDescription', [group_jid, description])

    def group_setting_update(self, group_jid, setting):
        return self._send_cmd('groupSettingUpdate', [group_jid, setting])

    def group_lock(self, group_jid):
        return self._send_cmd('groupSettingUpdate', [group_jid, 'announcement'])

    def group_unlock(self, group_jid):
        return self._send_cmd('groupSettingUpdate', [group_jid, 'not_announcement'])

    def group_lock_info(self, group_jid):
        return self._send_cmd('groupSettingUpdate', [group_jid, 'locked'])

    def group_unlock_info(self, group_jid):
        return self._send_cmd('groupSettingUpdate', [group_jid, 'unlocked'])

    def group_member_add_mode(self, group_jid, mode='admin'):
        return self._send_cmd('groupMemberAddMode', [group_jid, mode])

    def group_join_approval(self, group_jid, enabled=True):
        return self._send_cmd('groupJoinApproval', [group_jid, enabled])

    def group_request_list(self, group_jid):
        return self._send_cmd('groupRequestParticipantsList', [group_jid])

    def group_request_approve(self, group_jid, requester_jid):
        return self._send_cmd('groupRequestParticipantsUpdate', [group_jid, [requester_jid], 'approve'])

    def group_request_reject(self, group_jid, requester_jid):
        return self._send_cmd('groupRequestParticipantsUpdate', [group_jid, [requester_jid], 'reject'])

    def group_toggle_ephemeral(self, group_jid, duration=0):
        return self._send_cmd('groupToggleEphemeral', [group_jid, duration])

    def group_fetch_all(self):
        return self._send_cmd('groupFetchAllParticipating')

    def group_invite_info(self, code):
        return self._send_cmd('groupGetInviteInfo', [code])

    # ── Status / Stories ──
    def send_status_text(self, text):
        return self._send_cmd('sendStatusText', [text])

    def send_status_image(self, path, caption=''):
        return self._send_cmd('sendStatusImage', [path, caption])

    def send_status_video(self, path, caption=''):
        return self._send_cmd('sendStatusVideo', [path, caption])

    def fetch_status(self, *jids):
        return self._send_cmd('fetchStatus', list(jids))

    # ── Contacts ──
    def contacts(self):
        return self._send_cmd('getContacts')

    def block(self, jid):
        return self._send_cmd('updateBlock', [jid, 'block'])

    def unblock(self, jid):
        return self._send_cmd('updateBlock', [jid, 'unblock'])

    def blocklist(self):
        return self._send_cmd('fetchBlocklist')

    def on_whatsapp(self, *numbers):
        return self._send_cmd('onWhatsApp', list(numbers))

    def add_contact(self, jid, name=''):
        return self._send_cmd('addContact', [jid, name])

    def remove_contact(self, jid):
        return self._send_cmd('removeContact', [jid])

    # ── Profile ──
    def set_name(self, name):
        return self._send_cmd('updateProfileName', [name])

    def set_status(self, text):
        return self._send_cmd('updateProfileStatus', [text])

    def set_profile_picture(self, jid, path):
        return self._send_cmd('updateProfilePicture', [jid, path])

    def remove_profile_picture(self, jid='me'):
        return self._send_cmd('removeProfilePicture', [jid])

    def profile_picture(self, jid, type='image'):
        return self._send_cmd('profilePictureUrl', [jid, type])

    # ── Chat ──
    def chat_mute(self, jid, until=None):
        return self._send_cmd('chatModify', [jid, 'mute', until or 8 * 60 * 60 * 1000])

    def chat_unmute(self, jid):
        return self._send_cmd('chatModify', [jid, 'mute', None])

    def chat_archive(self, jid):
        return self._send_cmd('chatModify', [jid, 'archive'])

    def chat_unarchive(self, jid):
        return self._send_cmd('chatModify', [jid, 'unarchive'])

    def chat_pin(self, jid):
        return self._send_cmd('chatModify', [jid, 'pin'])

    def chat_unpin(self, jid):
        return self._send_cmd('chatModify', [jid, 'unpin'])

    def chat_mark_read(self, jid, msg_id=None, participant=None):
        if msg_id:
            return self._send_cmd('readMessages', [jid, msg_id, participant or ''])
        return self._send_cmd('chatModify', [jid, 'markAsRead'])

    def chat_mark_unread(self, jid):
        return self._send_cmd('chatModify', [jid, 'markAsUnread'])

    def chat_clear(self, jid):
        return self._send_cmd('chatModify', [jid, 'clear'])

    def chat_delete(self, jid):
        return self._send_cmd('chatModify', [jid, 'delete'])

    # ── Presence ──
    def presence(self, status='available'):
        return self._send_cmd('sendPresenceUpdate', [status])

    def typing(self, jid):
        return self._send_cmd('typingPresence', [jid])

    def recording(self, jid):
        return self._send_cmd('recordingPresence', [jid])

    # ── Privacy ──
    def privacy_settings(self):
        return self._send_cmd('fetchPrivacySettings')

    def set_last_seen_privacy(self, value='all'):
        return self._send_cmd('updateLastSeenPrivacy', [value])

    def set_read_receipts_privacy(self, value='all'):
        return self._send_cmd('updateReadReceiptsPrivacy', [value])

    def set_online_privacy(self, value='all'):
        return self._send_cmd('updateOnlinePrivacy', [value])

    def set_profile_picture_privacy(self, value='all'):
        return self._send_cmd('updateProfilePicturePrivacy', [value])

    def set_status_privacy(self, value='all'):
        return self._send_cmd('updateStatusPrivacy', [value])

    def set_groups_add_privacy(self, value='all'):
        return self._send_cmd('updateGroupsAddPrivacy', [value])

    def set_messages_privacy(self, value='all'):
        return self._send_cmd('updateMessagesPrivacy', [value])

    def set_call_privacy(self, value='all'):
        return self._send_cmd('updateCallPrivacy', [value])

    def set_link_previews(self, enabled=True):
        return self._send_cmd('updateDisableLinkPreviewsPrivacy', [not enabled])

    # ── Message History ──
    def load_messages(self, jid, count=10, cursor=None, msg_id=None):
        return self._send_cmd('loadMessages', [jid, count, cursor or '', msg_id or ''])

    # ── Newsletters / Channels ──
    def newsletter_create(self, name, description='', picture_path=None):
        return self._send_cmd('newsletterCreate', [name, description, picture_path or ''])

    def newsletter_follow(self, jid):
        return self._send_cmd('newsletterFollow', [jid])

    def newsletter_unfollow(self, jid):
        return self._send_cmd('newsletterUnfollow', [jid])

    def newsletter_metadata(self, jid):
        return self._send_cmd('newsletterMetadata', [jid])

    def newsletter_fetch_messages(self, jid, count=10):
        return self._send_cmd('newsletterFetchMessages', [jid, count])

    def newsletter_update_name(self, jid, name):
        return self._send_cmd('newsletterUpdateName', [jid, name])

    def newsletter_update_description(self, jid, description):
        return self._send_cmd('newsletterUpdateDescription', [jid, description])

    def newsletter_update_picture(self, jid, path):
        return self._send_cmd('newsletterUpdatePicture', [jid, path])

    def newsletter_remove_picture(self, jid):
        return self._send_cmd('newsletterRemovePicture', [jid])

    def newsletter_delete(self, jid):
        return self._send_cmd('newsletterDelete', [jid])

    def newsletter_mute(self, jid):
        return self._send_cmd('newsletterMute', [jid])

    def newsletter_unmute(self, jid):
        return self._send_cmd('newsletterUnmute', [jid])

    def newsletter_subscribers(self, jid):
        return self._send_cmd('newsletterSubscribers', [jid])

    def newsletter_admin_count(self, jid):
        return self._send_cmd('newsletterAdminCount', [jid])

    def newsletter_change_owner(self, jid, new_owner):
        return self._send_cmd('newsletterChangeOwner', [jid, new_owner])

    def newsletter_demote(self, jid, admin):
        return self._send_cmd('newsletterDemote', [jid, admin])

    def newsletter_react(self, jid, msg_id, emoji):
        return self._send_cmd('newsletterReactMessage', [jid, msg_id, emoji])

    def newsletter_update(self, jid, updates):
        return self._send_cmd('newsletterUpdate', [jid, updates])

    def newsletter_subscribe_updates(self, jids):
        return self._send_cmd('subscribeNewsletterUpdates', [list(jids)])

    # ── Business ──
    def business_profile(self, jid):
        return self._send_cmd('getBusinessProfile', [jid])

    # ── User ──
    def user(self):
        return self._send_cmd('getUser')

    # ── Callbacks ──
    def on(self, event, callback):
        self._callbacks[event] = callback

    def get_last_qr(self):
        return self._last_qr

    def clear_last_qr(self):
        self._last_qr = None
        try:
            self._send_cmd('clearLastQR', timeout=2)
        except Exception:
            pass

    def __setattr__(self, name, value):
        if name.startswith('on_'):
            event = name[3:]
            self._callbacks[event] = value
            if event == 'qr' and value and self._last_qr:
                value({'qr': self._last_qr})
        else:
            object.__setattr__(self, name, value)

    def _handle_event(self, name, data):
        if name == 'qr':
            self._last_qr = data.get('qr') if data else None
        cb = self._callbacks.get(name)
        if cb:
            cb(data)


def _reader_loop(proc, conn):
    while True:
        try:
            line = proc.stdout.readline()
            if not line:
                break
            line = line.decode('utf-8').strip()
            if not line:
                continue
            msg = json.loads(line)
            if msg.get('type') == 'response':
                rid = msg.get('id')
                if rid in conn._pending:
                    if 'error' in msg:
                        conn._pending[rid].set_error(msg['error'])
                    else:
                        conn._pending[rid].set(msg.get('result'))
            elif msg.get('type') == 'event':
                name = msg.get('name')
                data = msg.get('data')
                if name == 'qr_display':
                    print("\nScan QR with WhatsApp (Linked Devices -> Link a Device):\n")
                    print(data.get('text', ''))
                    print()
                else:
                    conn._handle_event(name, data)
        except Exception:
            break
    proc.wait()


def _connect(auth_dir='wa_auth', pairing_phone=None):
    try:
        proc = subprocess.Popen(
            ['node', _BRIDGE_SCRIPT],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
        )
    except FileNotFoundError:
        raise RuntimeError("Node.js is required. Install from https://nodejs.org")
    conn = _WAConnection(proc, None)
    reader = threading.Thread(target=_reader_loop, args=(proc, conn), daemon=True)
    conn._reader = reader
    reader.start()
    conn.connect(auth_dir, pairing_phone)
    return conn


def _build_wa_module():
    return {
        'connect': lambda auth_dir=None, pairing_phone=None: _connect(auth_dir or 'wa_auth', pairing_phone),
        'Connection': _WAConnection,
    }
