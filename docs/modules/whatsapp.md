# WhatsApp (wa) Module

Complete reference for WhatsApp integration using the Baileys library (unofficial WhatsApp Web API). Requires **Node.js** for the bridge process.

## Quick Start

```
// Connect and send a message
load wa
conn = wa.connect()

// QR code prints in terminal — scan with WhatsApp
// (Linked Devices → Link a Device)

conn.on_connected = function(data) {
    print "Connected as: " + data.jid
    conn.send("1234567890@s.whatsapp.net", "Hello from Zen!")
}
```

---

## Loading

```
load wa
```

This adds `wa.connect` (function) and `wa.Connection` (class) to your scope.

---

## Authentication

### QR Code Login (default)

Credentials are saved to the `wa_auth/` directory (or path you pass to `wa.connect()`). On subsequent connections, the saved session is reused automatically — no QR scan needed.

```
conn = wa.connect("wa_auth")
// QR code prints in terminal
// Scan with WhatsApp (Linked Devices → Link a Device)
```

### Handle QR Programmatically

```
conn = wa.connect()

conn.on_qr = function(data) {
    print "QR data: " + data.qr    // base64 string
}
```

Even if you set `on_qr` after connecting, the last QR is replayed to your callback. Use `conn.get_last_qr()` to retrieve it at any time, and `conn.clear_last_qr()` to clear the stored QR.

### Pairing Code Auth (no QR)

Instead of a QR code, authenticate by entering your phone number:

```
conn = wa.connect()
let code = conn.request_pairing_code("254712345678")
print "Enter this code in WhatsApp: " + code
```

Then open WhatsApp → Linked Devices → Link a Device → Pair with code.

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
conn.logout()    // invalidates saved credentials
```

After logout (or if credentials expire), a new QR will be printed on the next connection.

---

## Connection Management

| Method | Description |
|--------|-------------|
| `wa.connect(auth_dir?)` | Start connection, returns Connection object |
| `conn.disconnect()` | Disconnect and kill the bridge process |
| `conn.logout()` | Log out (invalidates saved credentials) |
| `conn.get_last_qr()` | Get last QR code (base64 string or null) |
| `conn.clear_last_qr()` | Clear stored QR |
| `conn.request_pairing_code(phone)` | Get pairing code for phone auth |
| `conn.connection_state()` | Get state: `idle`, `connecting`, `awaiting_qr`, `connected`, `disconnected`, `dead` |
| `conn.export_auth(password?, auth_dir?)` | Export encrypted auth blob |
| `conn.import_auth(password?, auth_dir?, payload)` | Restore auth from encrypted blob |

### Check connection state

```
let state = conn.connection_state()
print state    // "connected"

if state != "connected" {
    print "Not connected yet"
}
```

---

## Sending Messages

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

### Basic text message

```
conn.send("1234567890@s.whatsapp.net", "Hello!")
```

### Send image with caption

```
conn.send_image("1234567890@s.whatsapp.net", "/path/to/photo.jpg", "Check this out!")
```

### Send poll

```
let options = ["Option A", "Option B", "Option C"]
conn.send_poll("1234567890@s.whatsapp.net", "Vote for your favorite:", options)
```

### Reply to a message

```
conn.on_message = function(msg) {
    if msg.text == "help" {
        conn.send_reply(msg.from, "Sure! How can I help?", msg.id)
    }
}
```

### React to a message

```
conn.send_reaction(msg.from, msg.id, "👍")
```

### Forward a message

```
conn.forward_message("0987654321@s.whatsapp.net", msg.from, msg.id)
```

### Load message history

```
let msgs = conn.load_messages("1234567890@s.whatsapp.net", 20)
for msg in msgs {
    print "[" + msg.key.remoteJid + "] " + (msg.message?.conversation || "[media]")
}
```

---

## Group Management

All group methods take a group JID (e.g. `1234567890-123456@g.us`).

| Method | Description |
|--------|-------------|
| `conn.group_create(name, participants)` | Create group, returns group JID |
| `conn.group_add(group, participant)` | Add participant |
| `conn.group_remove(group, participant)` | Remove participant |
| `conn.group_promote(group, participant)` | Promote to admin |
| `conn.group_demote(group, participant)` | Demote from admin |
| `conn.group_leave(group)` | Leave group |
| `conn.group_info(group)` | Get metadata (name, desc, participants) |
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

### Create a group

```
let participants = [
    "1234567890@s.whatsapp.net",
    "0987654321@s.whatsapp.net"
]
let group = conn.group_create("Study Group", participants)
print "Group created: " + group
```

### Manage group settings

```
conn.group_set_description(group, "Welcome to the study group!")
conn.group_set_subject(group, "New Group Name")
conn.group_lock(group)    // only admins can send
```

### List all groups

```
let groups = conn.group_fetch_all()
for g in groups {
    print g.subject + " (" + g.id + ")"
}
```

### Get group info

```
let info = conn.group_info(group)
print "Name: " + info.subject
print "Description: " + info.desc
print "Participants: " + str(info.participants.len)
```

---

## Status / Stories

| Method | Description |
|--------|-------------|
| `conn.send_status_text(text)` | Post text status |
| `conn.send_status_image(path, caption?)` | Post image status |
| `conn.send_status_video(path, caption?)` | Post video status |
| `conn.fetch_status(*jids)` | Fetch statuses of contacts |

```
conn.send_status_text("Good morning! ☀️")
conn.send_status_image("/path/to/photo.jpg", "Beautiful sunset")
```

---

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

### Check if number is on WhatsApp

```
let result = conn.on_whatsapp("1234567890")
if result.exists {
    print "Number is on WhatsApp"
} else {
    print "Number not found"
}
```

### List contacts

```
let contacts = conn.contacts()
for c in contacts {
    print c.name + ": " + c.jid
}
```

---

## Profile

| Method | Description |
|--------|-------------|
| `conn.set_name(name)` | Update display name |
| `conn.set_status(text)` | Update profile status text |
| `conn.set_profile_picture(jid, path)` | Set profile/group picture |
| `conn.remove_profile_picture(jid?)` | Remove profile picture |
| `conn.profile_picture(jid, type?)` | Get profile picture URL |

```
conn.set_name("My Zen Bot")
conn.set_status("Powered by Zen 🚀")
conn.set_profile_picture("1234567890@s.whatsapp.net", "/path/to/pic.jpg")
```

---

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

```
conn.chat_mute("1234567890@s.whatsapp.net")
conn.chat_pin("1234567890@s.whatsapp.net")
conn.chat_mark_read("1234567890@s.whatsapp.net")
```

---

## Presence

| Method | Description |
|--------|-------------|
| `conn.presence(status?)` | Set online status (`available` / `unavailable`) |
| `conn.typing(jid)` | Show typing indicator in chat |
| `conn.recording(jid)` | Show recording indicator in chat |

```
conn.typing("1234567890@s.whatsapp.net")
// ... compose message ...
conn.send("1234567890@s.whatsapp.net", "Here's my reply!")
```

---

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

```
conn.set_last_seen_privacy("contacts")
conn.set_read_receipts_privacy("none")
conn.set_online_privacy("contacts")
```

---

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

### Create and manage a channel

```
let channel = conn.newsletter_create("My Channel", "News and updates")
conn.newsletter_follow(channel)

let metadata = conn.newsletter_metadata(channel)
print "Subscribers: " + str(metadata.subscribers_count)
```

---

## Callbacks

Set callbacks to react to events:

```
conn.on_qr = function(data) {
    print "QR: " + data.qr
}

conn.on_connected = function(data) {
    print "Connected: " + data.jid
}

conn.on_disconnected = function(data) {
    print "Disconnected: " + str(data.reason)
}

conn.on_message = function(msg) {
    print "From: " + msg.from
    print "Text: " + msg.text
    print "Type: " + msg.type
}

conn.on_presence = function(data) {
    print data.jid + " is " + data.status
}
```

You can also use `conn.on("event_name", callback)` or shorthand dot-assignment: `conn.on_qr = callback` is equivalent to `conn.on("qr", callback)`.

### Available events

| Event | Data Fields |
|-------|-------------|
| `qr` | `qr` (base64 string) |
| `connected` | `jid` (own phone JID) |
| `disconnected` | `reason` (disconnect reason code) |
| `message` | `from`, `id`, `text`, `sender`, `type`, `message` |
| `presence` | `jid`, `status` (`online`/`offline`/`composing`) |

---

## Business

| Method | Description |
|--------|-------------|
| `conn.business_profile(jid)` | Get business profile info |
| `conn.user()` | Get own user info |

---

## Error Handling

Connection and network errors raise `Runtime Error` in Zen. Methods that fail on the bridge side (e.g. invalid JID, network timeout) return an error string. Use `try`/`catch` to handle gracefully:

```
try {
    conn.send("invalidjid@s.whatsapp.net", "Hello")
} catch err {
    print "Send failed: " + err
}
```

---

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
conn.group_lock(group)    // only admins can send
```

### Broadcast Status Update

```
load wa
conn = wa.connect()

conn.send_status_text("Good morning, Zen community! ☀️")
conn.send_status_image("/path/to/photo.jpg", "Beautiful sunrise today")
```

### Message Logger

```
load wa
conn = wa.connect()

conn.on_message = function(msg) {
    let entry = datetime.now() + " | " + msg.from + " | " + msg.text
    fs.append("whatsapp_log.txt", entry + "\n")
}

print "Logging messages..."
```

### Auto-Responder with Keywords

```
load wa
conn = wa.connect()

conn.on_message = function(msg) {
    let text = msg.text.lower()

    if text == "help" {
        conn.send(msg.from, "Available commands: help, status, time")
    } else if text == "status" {
        conn.send(msg.from, "All systems operational ✅")
    } else if text == "time" {
        conn.send(msg.from, "Current time: " + datetime.now())
    }
}
```

---

## Pro Tips

1. **Use pairing code for headless setups.** No QR scanning needed on remote servers.
2. **Export encrypted sessions.** `export_auth()` saves credentials securely.
3. **Handle reconnection.** Set `on_disconnected` to reconnect automatically.
4. **Use `try/catch` for send operations.** Network errors are common.
5. **Don't spam messages.** WhatsApp may ban your number for excessive sending.

---

## Common Mistakes

### Wrong JID format

```
// WRONG — missing @s.whatsapp.net
conn.send("1234567890", "Hello")

// CORRECT
conn.send("1234567890@s.whatsapp.net", "Hello")
```

### Not waiting for connection

```
// WRONG — connection might not be ready
conn.send("1234567890@s.whatsapp.net", "Hello")

// CORRECT — wait for connected event
conn.on_connected = function(data) {
    conn.send("1234567890@s.whatsapp.net", "Hello")
}
```

### Sending too fast

```
// BAD — may trigger rate limits
for i in 1 -> 100 {
    conn.send("1234567890@s.whatsapp.net", "Message " + str(i))
}

// GOOD — add delays
for i in 1 -> 100 {
    conn.send("1234567890@s.whatsapp.net", "Message " + str(i))
    sleep(1)    // 1 second between messages
}
```

---

## See Also

- [Module Overview](overview.md) — All available modules
- [HTTP Module](http.md) — Web requests
- [os Module](overview.md) — Process and environment info
