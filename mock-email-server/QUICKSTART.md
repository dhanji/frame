# Quick Start Guide

## Starting the Mock Server

```bash
python3 mock_server.py
```

The server will start both IMAP and SMTP services.

## Connecting from Frame Email Client

Use these credentials in your email client:

- **Email**: `test@example.com`
- **Password**: `password123` (or any password - the mock server accepts all)
- **IMAP Host**: `localhost`
- **IMAP Port**: `1143`
- **SMTP Host**: `localhost`
- **SMTP Port**: `1587`

## Test Data

The server comes pre-loaded with:
- 10 test emails
- 6 conversation threads
- Multiple senders (alice, bob, charlie, dave, eve, frank, grace)
- Various timestamps

## Features

### IMAP Server (Port 1143)
- LOGIN - accepts any credentials
- LIST - returns folder structure
- SELECT - selects INBOX
- FETCH - retrieves emails
- SEARCH - searches emails
- LOGOUT - closes connection

### SMTP Server (Port 1587)
- HELO/EHLO - greeting
- MAIL FROM - set sender
- RCPT TO - set recipient
- DATA - send email content
- QUIT - close connection

## Testing Tips

1. **Test Conversation Threading**: The emails are pre-grouped into threads using the `thread_id` field
2. **Test Sending**: Send emails through the SMTP server - they'll be logged to console
3. **Test Search**: All emails contain searchable content
4. **Test Multiple Accounts**: You can modify `test_data.json` to add more users

## Customizing Test Data

Edit `test_data.json` to:
- Add more emails
- Create new conversation threads
- Add more folders
- Add more test users

## Troubleshooting

### Port Already in Use
If ports 1143 or 1587 are already in use, edit `mock_server.py` and change:
```python
imap_server = MockIMAPServer(host='localhost', port=YOUR_PORT)
smtp_server = MockSMTPServer(host='localhost', port=YOUR_PORT)
```

### Connection Refused
Make sure:
1. The mock server is running
2. No firewall is blocking the ports
3. You're using `localhost` as the host

### Emails Not Showing
The mock server logs all activity to console. Check the output for:
- Connection attempts
- Authentication
- Commands received
- Errors

## Advanced Usage

### Running on Different Host
To allow connections from other machines:
```python
imap_server = MockIMAPServer(host='0.0.0.0', port=1143)
smtp_server = MockSMTPServer(host='0.0.0.0', port=1587)
```

### Adding More Emails Dynamically
The server stores emails in memory. You can modify the `TEST_EMAILS` list in `mock_server.py` or load from `test_data.json`.

## Comparison with Real Email Servers

### Advantages of Mock Server
✅ No external dependencies
✅ Works offline
✅ Instant setup
✅ Full control over test data
✅ No rate limits
✅ No security concerns
✅ Reproducible tests

### Limitations
❌ Simplified IMAP/SMTP implementation
❌ No SSL/TLS support (use plain connections)
❌ No persistent storage (emails lost on restart)
❌ Limited command support

### When to Use Real Email Server
- Testing SSL/TLS connections
- Testing with real email providers
- Long-term testing
- Testing edge cases in IMAP/SMTP protocols

## Next Steps

1. Start the mock server
2. Configure Frame Email Client with the test credentials
3. Test basic operations (login, fetch, send)
4. Test conversation threading
5. Test search functionality
6. Add custom test data as needed

## Support

For issues or questions:
1. Check the console output for errors
2. Verify your client configuration
3. Review the test data in `test_data.json`
4. Check the README.md for more details
