# Mock IMAP/SMTP Server for Testing

This is a lightweight Python-based mock email server that implements basic IMAP and SMTP protocols for testing the Frame Email Client.

## Features

- **Mock IMAP Server**: Responds to IMAP commands with test data
- **Mock SMTP Server**: Accepts emails and logs them
- **Pre-populated Test Data**: Includes sample conversations and threads
- **No External Dependencies**: Runs completely locally (pure Python)
- **Easy to Extend**: Add custom test scenarios
- **Console Logging**: See all server activity in real-time
- **Thread-safe**: Handles multiple concurrent connections

## Quick Start

### 1. Start the Mock Server

```bash
cd tmp/mock-email-server
python3 mock_server.py
```

This will start:
- IMAP server on port 1143
- SMTP server on port 1587

### 2. Test the Connection (Optional)

In another terminal:
```bash
python3 test_connection.py
```

### 3. Configure Frame Email Client

Use these credentials:

- **Email**: `test@example.com`
- **Password**: `password123` (any password works)
- **IMAP Host**: `localhost`
- **IMAP Port**: `1143`
- **SMTP Host**: `localhost`
- **SMTP Port**: `1587`

## Test Data

The mock server comes pre-populated with:
- **10 test emails** organized into 6 conversation threads
- **Multiple senders**: alice, bob, charlie, dave, eve, frank, grace
- **Various timestamps** to test sorting and display
- **Thread relationships** to test conversation grouping

Edit `test_data.json` to customize the test data.

## Supported Commands

### IMAP Commands
- `CAPABILITY` - Returns server capabilities
- `LOGIN` - Authenticates user (accepts any credentials)
- `LIST` - Lists available folders
- `SELECT` - Selects a mailbox (INBOX)
- `FETCH` - Retrieves email messages
- `SEARCH` - Searches for messages
- `LOGOUT` - Closes connection

### SMTP Commands
- `HELO/EHLO` - Greeting
- `MAIL FROM` - Sets sender
- `RCPT TO` - Sets recipient
- `DATA` - Sends email content
- `QUIT` - Closes connection

## Testing with Frame Email Client

### Step-by-Step

1. **Start the mock server** (in terminal 1):
   ```bash
   cd tmp/mock-email-server
   python3 mock_server.py
   ```

2. **Start Frame backend** (in terminal 2):
   ```bash
   cd backend
   cargo run
   ```

3. **Start Frame frontend** (in terminal 3):
   ```bash
   cd frontend
   npm run dev
   ```

4. **Open browser** and navigate to the frontend URL (usually http://localhost:3000)

5. **Login** with the test credentials listed above

### What You Can Test

#### ✅ Conversation Threading
- View emails organized by conversation threads
- Expand/collapse thread views
- See the latest messages in each thread
- Test the feed-like interface

#### ✅ Email Operations
- Fetch and display emails from INBOX
- View email details (from, to, subject, date, body)
- Test email rendering

#### ✅ Sending Emails
- Compose new emails
- Reply to existing emails
- Send emails (logged to server console)

#### ✅ Search Functionality
- Search across all emails
- Find specific senders, subjects, or content

#### ✅ Multiple Connections
- Test with multiple browser tabs
- Concurrent access from multiple clients

## Advantages Over Real Email Servers

| Feature | Mock Server | Real Server (Gmail) |
|---------|-------------|---------------------|
| **Setup Time** | < 1 minute | 10-30 minutes |
| **Dependencies** | None (pure Python) | Internet, Account, App Password |
| **Rate Limits** | None | Yes (strict) |
| **Offline Testing** | ✅ Yes | ❌ No |
| **Custom Test Data** | ✅ Easy | ❌ Manual |
| **Reproducible** | ✅ Yes | ❌ No |
| **Security** | ✅ No credentials needed | ⚠️ Real credentials |
| **Cost** | ✅ Free | ✅ Free |
| **Speed** | ✅ Instant | ⚠️ Network latency |

## Troubleshooting

### Port Already in Use

If you see `Address already in use`, edit `mock_server.py` and change the port numbers:

```python
imap_server = MockIMAPServer(host='localhost', port=2143)  # Changed from 1143
smtp_server = MockSMTPServer(host='localhost', port=2587)  # Changed from 1587
```

Then update your client configuration to use the new ports.

### Connection Refused

Make sure:
1. ✓ The mock server is running (you should see startup messages)
2. ✓ You're using `localhost` as the host
3. ✓ You're using the correct ports (1143 for IMAP, 1587 for SMTP)
4. ✓ No firewall is blocking the connections

### Emails Not Appearing

Check the server console output for:
- Connection attempts from your client
- Authentication messages (LOGIN command)
- IMAP commands being received (SELECT, FETCH, etc.)
- Any error messages

The server logs all activity, so you can see exactly what's happening.

### Python Version Issues

This server requires Python 3.6 or higher. Check your version:
```bash
python3 --version
```

If you have an older version, install Python 3.6+.

## Customization

### Adding More Emails

Edit `test_data.json` and add entries to the `emails` array:

```json
{
  "id": 11,
  "from": "newuser@example.com",
  "to": "test@example.com",
  "subject": "New Test Email",
  "date": "Fri, 19 Jan 2024 10:00:00 +0000",
  "body": "This is a new test email for testing.",
  "thread_id": "thread-7"
}
```

### Creating New Conversation Threads

Use the same `thread_id` for emails that should be grouped together:

```json
[
  {
    "id": 11,
    "subject": "New Discussion",
    "thread_id": "thread-7"
  },
  {
    "id": 12,
    "subject": "Re: New Discussion",
    "thread_id": "thread-7"
  }
]
```

### Adding More Users

Edit the `users` array in `test_data.json`:

```json
{
  "email": "another@example.com",
  "password": "test123",
  "name": "Another User"
}
```

### Modifying Server Behavior

Edit `mock_server.py`:
- `MockIMAPServer.handle_client()` - Add/modify IMAP commands
- `MockSMTPServer.handle_client()` - Add/modify SMTP commands
- `TEST_EMAILS` - Change the default email data

## Architecture

### How It Works

1. **Two Servers**: Separate threads for IMAP and SMTP
2. **Socket-based**: Uses Python's `socket` library
3. **Multi-threaded**: Each client connection gets its own thread
4. **In-memory Storage**: Emails stored in Python lists (not persistent)
5. **Simple Protocol**: Implements minimal IMAP/SMTP for testing

### Limitations

- ❌ No SSL/TLS support (use plain connections)
- ❌ No persistent storage (emails lost on restart)
- ❌ Simplified protocol implementation
- ❌ Limited IMAP command support
- ❌ No authentication validation (accepts all passwords)

These limitations are intentional to keep the server simple and focused on testing.

## Files

- `mock_server.py` - Main server implementation (332 lines)
- `test_data.json` - Sample email data (customizable)
- `test_connection.py` - Connection test script
- `README.md` - This file
- `QUICKSTART.md` - Quick reference guide

## When to Use Real Email Server

Consider using a real email server (like Gmail) when:
- Testing SSL/TLS connections
- Testing with real email providers
- Testing long-term email storage
- Testing edge cases in IMAP/SMTP protocols
- Testing with real-world email formats

For development and basic testing, this mock server is recommended.

## Contributing

Feel free to extend this mock server:
- Add more IMAP commands
- Implement SSL/TLS support
- Add persistent storage
- Improve error handling
- Add more test scenarios

## License

This mock server is provided as-is for testing purposes. Feel free to modify and extend it for your needs.
