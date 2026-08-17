# WhatsApp (wa)

The `wa` module provides a full WhatsApp client using the [Baileys](https://github.com/WhiskeySockets/Baileys) library (unofficial WhatsApp Web API). Requires **Node.js** to run the bridge process.

## Loading

```
load wa
```

This adds `wa.connect` (function) and `wa.Connection` (class) to your scope.

## Quick Start

```
load wa
conn = wa.connect("wa_auth")

// QR code will print as ASCII art in the terminal
// Scan it with WhatsApp (Linked Devices → Link a Device)

conn.on_connected = function(data) {
    print "Connected as: " + data.jid
}

conn.on_message = function(msg) {
    print "From: " + msg.from
    print "Text: " + msg.text
}
```

## Authentication

Credentials are saved to the `wa_auth/` directory (or the path you pass to `wa.connect()`). On subsequent connections, the saved session is reused automatically — no QR scan needed.

### QR Display

The QR code prints as ASCII art directly to your terminal when a new login is required. You can also handle it programmatically:

```
conn = wa.connect()
conn.on_qr = function(data) {
    print "QR data: " + data.qr   // base64 string
}
```

Even if you set `on_qr` after connecting, the last QR is replayed to your callback. Use `conn.get_last_qr()` to retrieve it at any time, and `conn.clear_last_qr()` to clear the stored QR.

### Pairing Code Auth

Instead of a QR code, you can authenticate by entering your phone number:

```
conn = wa.connect()
let code = conn.request_pairing_code("254712345678")
print "Enter this code in WhatsApp: " + code
```

Open WhatsApp → Linked Devices → Link a Device → Pair with code.

### Encrypted Session Export

Save your session to an encrypted file for reuse:

```
conn = wa.connect()
// ... wait for connection ...
let encrypted = conn.export_auth("my-password")
fs.write("wa_session.enc", encrypted)

// Later:
conn = wa.connect()
conn.import_auth("my-password", "wa_auth", fs.read("wa_session.enc"))
```

### Logging Out

```
conn.logout()        // invalidate saved credentials
```

After logout (or if credentials expire), a new QR will be printed on the next connection.

## Connection

| Method | Description |
|--------|-------------|
| `wa.connect(auth_dir?)` | Start connection, returns a Connection object |
| `conn.disconnect()` | Disconnect and kill the bridge process |
| `conn.logout()` | Log out (invalidates saved credentials) |
| `conn.get_last_qr()` | Get the last QR code (base64 string or null) |
| `conn.clear_last_qr()` | Clear stored QR |
| `conn.request_pairing_code(phone)` | Get pairing code for phone number auth |
| `conn.connection_state()` | Get state: `idle`, `connecting`, `awaiting_qr`, `connected`, `disconnected`, `dead` |
| `conn.export_auth(password?, auth_dir?)` | Export encrypted auth blob |
| `conn.import_auth(password?, auth_dir?, payload)` | Restore auth from encrypted blob |

## Messaging

JIDs are WhatsApp identifiers: `{number}@s.whatsapp.net` for users, `{id}@g.us` for groups.

| Method | Description |
|--------|-------------|
| `conn.send(jid, text)` | Send text message |
| `conn.send_image(jid, path, caption?)` | Send image |
| `conn.send_video(jid, path, caption?)` | Send video |
| `conn.send_audio(jid, path, as_voice=true)` | Send audio (voice note by default) |
| `conn.send_sticker(jid, path)` | Send sticker (WebP) |
| `conn.send_document(jid, path, filename?)` | Send file |
| `conn.send_view_once(jid, path, type?, caption?)` | Send view-once image/video |
| `conn.send_contact(jid, name, vcard)` | Send contact card |
| `conn.send_location(jid, lat, lon, name?)` | Send location |
| `conn.send_poll(jid, question, options)` | Send poll (options is a list) |
| `conn.send_reaction(jid, msg_id, emoji)` | React to message |
| `conn.send_reply(jid, text, reply_msg_id)` | Reply to a message |
| `conn.forward_message(jid, from_jid, msg_id)` | Forward a message |
| `conn.delete_message(jid, msg_id)` | Delete a message |
| `conn.star_message(jid, msg_id, starred?)` | Star/unstar a message |
| `conn.load_messages(jid, count?, cursor?, msg_id?)` | Fetch message history |

## Groups

All group methods take a group JID (e.g. `1234567890-123456@g.us`).

| Method | Description |
|--------|-------------|
| `conn.group_create(name, participants)` | Create group, returns group JID |
| `conn.group_add(group, participant)` | Add participant |
| `conn.group_remove(group, participant)` | Remove participant |
| `conn.group_promote(group, participant)` | Promote to admin |
| `conn.group_demote(group, participant)` | Demote from admin |
| `conn.group_leave(group)` | Leave group |
| `conn.group_info(group)` | Get metadata (name, desc, participants, etc.) |
| `conn.group_invite_code(group)` | Get invite link code |
| `conn.group_revoke_invite(group)` | Revoke invite link |
| `conn.group_accept_invite(code)` | Join via invite code |
| `conn.group_set_subject(group, name)` | Update group name |
| `conn.group_set_description(group, desc)` | Update description |
| `conn.group_lock(group)` | Set to announcement mode (only admins send) |
| `conn.group_unlock(group)` | Set to open mode (all members send) |
| `conn.group_lock_info(group)` | Lock group info |
| `conn.group_unlock_info(group)` | Unlock group info |
| `conn.group_member_add_mode(group, mode)` | Set who can add members (`admin` or `all`) |
| `conn.group_join_approval(group, enabled?)` | Enable/disable join approval requests |
| `conn.group_request_list(group)` | List pending join requests |
| `conn.group_request_approve(group, jid)` | Approve a join request |
| `conn.group_request_reject(group, jid)` | Reject a join request |
| `conn.group_toggle_ephemeral(group, duration?)` | Set disappearing messages (seconds, 0=off) |
| `conn.group_fetch_all()` | List all joined groups |

## Status / Stories

| Method | Description |
|--------|-------------|
| `conn.send_status_text(text)` | Post text status |
| `conn.send_status_image(path, caption?)` | Post image status |
| `conn.send_status_video(path, caption?)` | Post video status |
| `conn.fetch_status(*jids)` | Fetch statuses of contacts |

## Contacts

| Method | Description |
|--------|-------------|
| `conn.contacts()` | List all contacts |
| `conn.block(jid)` | Block a contact |
| `conn.unblock(jid)` | Unblock a contact |
| `conn.blocklist()` | List blocked contacts |
| `conn.on_whatsapp(*numbers)` | Check if numbers are on WhatsApp |
| `conn.add_contact(jid, name?)` | Add/save a contact |
| `conn.remove_contact(jid)` | Remove a contact |

## Profile

| Method | Description |
|--------|-------------|
| `conn.set_name(name)` | Update display name |
| `conn.set_status(text)` | Update profile status text |
| `conn.set_profile_picture(jid, path)` | Set profile/group picture |
| `conn.remove_profile_picture(jid?)` | Remove profile picture |
| `conn.profile_picture(jid, type?)` | Get profile picture URL |

## Chat Operations

| Method | Description |
|--------|-------------|
| `conn.chat_mute(jid, until?)` | Mute chat (default 8 hours) |
| `conn.chat_unmute(jid)` | Unmute chat |
| `conn.chat_archive(jid)` | Archive chat |
| `conn.chat_unarchive(jid)` | Unarchive chat |
| `conn.chat_pin(jid)` | Pin chat |
| `conn.chat_unpin(jid)` | Unpin chat |
| `conn.chat_mark_read(jid, msg_id?)` | Mark as read |
| `conn.chat_mark_unread(jid)` | Mark as unread |
| `conn.chat_clear(jid)` | Clear chat messages |
| `conn.chat_delete(jid)` | Delete chat |

## Presence

| Method | Description |
|--------|-------------|
| `conn.presence(status?)` | Set online status (`available` / `unavailable`) |
| `conn.typing(jid)` | Show typing indicator in chat |
| `conn.recording(jid)` | Show recording indicator in chat |

## Privacy

| Method | Description |
|--------|-------------|
| `conn.privacy_settings()` | Get all privacy settings |
| `conn.set_last_seen_privacy(value)` | `all` / `contacts` / `contact_blacklist` / `none` |
| `conn.set_read_receipts_privacy(value)` | Same values |
| `conn.set_online_privacy(value)` | Same values |
| `conn.set_profile_picture_privacy(value)` | Same values |
| `conn.set_status_privacy(value)` | Same values |
| `conn.set_groups_add_privacy(value)` | Same values |
| `conn.set_messages_privacy(value)` | Same values |
| `conn.set_call_privacy(value)` | Same values |
| `conn.set_link_previews(enabled?)` | Enable/disable link previews |

## Newsletters (Channels)

| Method | Description |
|--------|-------------|
| `conn.newsletter_create(name, description?, picture?)` | Create a channel |
| `conn.newsletter_follow(jid)` | Follow a channel |
| `conn.newsletter_unfollow(jid)` | Unfollow a channel |
| `conn.newsletter_metadata(jid)` | Get channel metadata |
| `conn.newsletter_fetch_messages(jid, count?)` | Fetch channel messages |
| `conn.newsletter_update_name(jid, name)` | Update channel name |
| `conn.newsletter_update_description(jid, desc)` | Update description |
| `conn.newsletter_update_picture(jid, path)` | Update channel picture |
| `conn.newsletter_remove_picture(jid)` | Remove channel picture |
| `conn.newsletter_delete(jid)` | Delete channel |
| `conn.newsletter_mute(jid)` | Mute channel |
| `conn.newsletter_unmute(jid)` | Unmute channel |
| `conn.newsletter_subscribers(jid)` | Get subscriber count |
| `conn.newsletter_admin_count(jid)` | Get admin count |
| `conn.newsletter_change_owner(jid, new_owner)` | Transfer ownership |
| `conn.newsletter_demote(jid, admin)` | Demote an admin |
| `conn.newsletter_react(jid, msg_id, emoji)` | React to channel message |
| `conn.newsletter_update(jid, updates)` | Update channel settings |
| `conn.newsletter_subscribe_updates(jids)` | Subscribe to newsletter updates |

## Callbacks

Set callbacks to react to events:

```
conn.on_qr = function(data) {
    // data.qr — base64 QR string
}
conn.on_connected = function(data) {
    // data.jid — own phone JID
}
conn.on_disconnected = function(data) {
    // data.reason — disconnect reason code
}
conn.on_message = function(msg) {
    // msg.from — sender JID
    // msg.id — message ID
    // msg.text — message text
    // msg.sender — participant (groups)
    // msg.type — content type
}
conn.on_presence = function(data) {
    // data.jid — contact JID
    // data.status — "online" / "offline" / "composing"
}
```

You can also use `conn.on("event_name", callback)` or shorthand dot-assignment:
`conn.on_qr = callback` is equivalent to `conn.on("qr", callback)`.

## Business

| Method | Description |
|--------|-------------|
| `conn.business_profile(jid)` | Get business profile info |
| `conn.user()` | Get own user info |

## Error Handling

Connection and network errors raise `Runtime Error` in Zen. Methods that fail on the bridge side (e.g. invalid JID, network timeout) return an error string. Use `try`/`catch` to handle gracefully:

```
try {
    conn.send("invalidjid@s.whatsapp.net", "Hello")
} catch err {
    print "Send failed: " + err
}
```

## Examples

### Auto-Reply Bot

```
load wa
conn = wa.connect()

conn.on_message = function(msg) {
    if "hello" in msg.text.lower() {
        conn.send(msg.from, "Hi there! I'm a Zen bot 🤖")
    }
}

print "Bot running. Press Ctrl+C to stop."
```

### Group Management

```
load wa
conn = wa.connect("wa_auth")
let group = conn.group_create("Study Group", ["254712345678@s.whatsapp.net"])
print "Group created: " + group
conn.group_set_description(group, "Welcome to the study group!")
conn.group_lock(group)  // only admins can send
```

### Broadcast Status Update

```
load wa
conn = wa.connect()
conn.send_status_text("Good morning, Zen community! ☀️")
conn.send_status_image("/path/to/photo.jpg", "Beautiful sunrise today")
```

### Fetch Message History

```
load wa
conn = wa.connect()
let msgs = conn.load_messages("254712345678@s.whatsapp.net", 20)
for msg in msgs {
    print "[" + msg.key.remoteJid + "] " + (msg.message?.conversation || "[media]")
}
```
