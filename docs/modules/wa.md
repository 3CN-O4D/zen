# wa — WhatsApp automation

The `wa` module provides tools for automating WhatsApp through a built-in client. It is available globally as `wa`.

```zen
# 1. Connect and show QR code
wa.connect("auth_session")
print(wa.qr())

# 2. Wait for connection
while wa.state() != "CONNECTED" {
    sleep(1)
}

# 3. Send a message
wa.sendText("1234567890@s.whatsapp.net", "Hello from Zen!")
```

## Functions

| Function | Description |
|----------|-------------|
| `connect(dir)` | Initializes the client using the specified directory for authentication data. |
| `qr()` | Returns the current QR code as a string (if not authenticated). |
| `pairingCode(num)` | Generates a pairing code for the specified phone number. |
| `state()` | Returns the current connection state (e.g., "CONNECTED", "DISCONNECTED"). |
| `sendText(jid, text)` | Sends a text message to a JID. |
| `send(jid, content)` | Sends a complex message (e.g., with media). |
| `poll()` | Polls for new events/messages. |
| `logout()` | Logs out the current session. |
| `disconnect()` | Closes the connection. |

## Working with JIDs
WhatsApp uses JIDs (Jabber Identifiers) to identify users and groups:
- Users: `[number]@s.whatsapp.net` (e.g., `1234567890@s.whatsapp.net`)
- Groups: `[id]@g.us`

## Examples

### Checking connection state
```zen
var status = wa.state()
print("Current WhatsApp status: ${status}")
```

### Logging out
```zen
if wa.state() == "CONNECTED" {
    wa.logout()
    print("Logged out successfully.")
}
```

## See Also
- [browser](browser/overview.md) — For web-based automation.
- [threading](threading.md) — For running a WhatsApp poll loop in the background.
