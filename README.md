# Frame Email Client

A modern email client with a conversation-based interface, similar to Facebook's feed design. 

**Built with:**
- **Backend:** Rust (Actix-web, SQLite, IMAP/SMTP)
- **Frontend:** Standalone HTML/JavaScript (no build tools required!)

**Key Feature:** The entire frontend is a single HTML file - no npm, no webpack, no build step. Just start the Rust backend and go!

📚 **[Quick Start Guide](QUICKSTART.md)** | 🎨 **[Frontend Architecture](FRONTEND.md)**

## Features

### Core Functionality
- **Conversation Threading**: Emails grouped by conversation threads
- **Inline Reply**: Reply directly from the conversation view
- **Multi-Account Support**: Connect via IMAP/SMTP
- **Real-time Updates**: WebSocket support for instant notifications
- **Search**: Full-text search across emails
- **Folder Management**: Create and organize custom folders
- **Bulk Operations**: Select and manage multiple emails at once

### User Interface
- **Feed-like Display**: Conversations shown like social media posts
- **Preview Messages**: See the last 2-3 messages in each thread
- **Expandable Threads**: Click to view full conversation history
- **Responsive Design**: Works on desktop, tablet, and mobile
- **Dark Mode Ready**: Clean, modern interface

## Architecture

### Backend (Rust)
- **Web Framework**: Actix-web
- **Database**: SQLite with SQLx
- **Email Protocols**: IMAP for receiving, SMTP for sending
- **Authentication**: JWT tokens
- **WebSocket**: Real-time updates

### Frontend (Standalone HTML)
- **Standalone HTML**: Single-file application with vanilla JavaScript
- **No Build Required**: Works directly without npm, webpack, or any build tools
- **Modern UI**: Clean, responsive design inspired by social media feeds
- **Full Features**: Reply, Forward, Search, Keyboard shortcuts, and more

## Installation & Setup

### Prerequisites
- Rust (latest stable version)
- **That's it!** No Node.js, npm, or any other tools required.

### Backend Setup

1. Navigate to the backend directory:
```bash
cd email-client/backend
```

2. Create the database and run migrations:
```bash
# Install SQLx CLI if not already installed
cargo install sqlx-cli --no-default-features --features sqlite

# Create database
sqlx database create

# Run migrations
sqlx migrate run
```

3. Configure environment variables:
```bash
# Copy the .env file and update with your settings
cp .env.example .env
```

4. Build and run the backend server:
```bash
cargo build --release
cargo run --release
```

The backend server will start on `http://localhost:8080`

### Frontend (No Setup Required!)

The frontend is a standalone HTML file that's automatically served by the Rust backend.
No installation or build steps required!

Simply start the backend and access the app at `http://localhost:8080`

### Production Deployment

**Quick Deploy:**
```bash
cd backend
cargo build --release
./target/release/email-server
```

#### Build and Run Backend
```bash
cd email-client/backend
cargo build --release
./target/release/email-server
```

#### Frontend
The frontend is already production-ready! The `app-working.html` file is a fully optimized
standalone application that requires no build step. Just ensure the backend serves it correctly.

The backend automatically serves `frontend/app-working.html` as the index page.

## Configuration

### Email Account Setup

When logging in, you'll need to provide:
- **Email Address**: Your email address
- **Password**: Your email password
- **IMAP Host**: e.g., `imap.gmail.com`
- **IMAP Port**: Usually `993` for SSL/TLS
- **SMTP Host**: e.g., `smtp.gmail.com`
- **SMTP Port**: Usually `587` for STARTTLS

### Gmail Configuration

For Gmail accounts:
1. Enable "Less secure app access" or use App Passwords
2. Enable IMAP in Gmail settings
3. Use these settings:
   - IMAP Host: `imap.gmail.com`
   - IMAP Port: `993`
   - SMTP Host: `smtp.gmail.com`
   - SMTP Port: `587`

## API Documentation

### Authentication

#### Login
```http
POST /api/auth/login
Content-Type: application/json

{
  "email": "user@example.com",
  "password": "password",
  "imap_host": "imap.gmail.com",
  "imap_port": 993,
  "smtp_host": "smtp.gmail.com",
  "smtp_port": 587
}
```

### Conversations

#### Get Conversations
```http
GET /api/conversations?folder_id={id}&limit=50&offset=0
Authorization: Bearer {token}
```

#### Get Conversation Thread
```http
GET /api/conversations/{thread_id}
Authorization: Bearer {token}
```

### Emails

#### Send Email
```http
POST /api/emails/send
Authorization: Bearer {token}
Content-Type: application/json

{
  "to": ["recipient@example.com"],
  "cc": ["cc@example.com"],
  "subject": "Subject",
  "body_text": "Email content"
}
```

#### Reply to Email
```http
POST /api/emails/{id}/reply
Authorization: Bearer {token}
Content-Type: application/json

{
  "email_id": "uuid",
  "reply_all": false,
  "body_text": "Reply content"
}
```

## Development

### Frontend Development

The frontend is `frontend/app-working.html` - a single HTML file with embedded CSS and JavaScript.

**To modify:**
1. Edit `frontend/app-working.html`
2. Save the file
3. Refresh your browser
4. Done!

See **[FRONTEND.md](FRONTEND.md)** for detailed architecture documentation.


### Database Migrations

Create a new migration:
```bash
sqlx migrate add <migration_name>
```

### Backend Development

```bash
cd backend
cargo watch -x run  # Auto-reload on changes
cargo test          # Run tests
```

### Running Tests

#### Backend Tests
```bash
cd email-client/backend
cargo test
```

### Code Structure

```
email-client/
├── backend/
│   ├── src/
│   │   ├── handlers/     # HTTP request handlers
│   │   ├── services/     # Business logic
│   │   ├── models.rs     # Data models
│   │   ├── error.rs      # Error handling
│   │   └── main.rs       # Application entry point
│   ├── migrations/       # Database migrations
│   └── Cargo.toml       # Rust dependencies
├── frontend/
│   ├── app-working.html # Main application (standalone, no build required)
│   ├── src/             # TypeScript version (optional, not used by default)
│   └── package.json     # Optional dependencies for TypeScript development
└── README.md
```

## Security Considerations

1. **Password Storage**: User email passwords are currently stored encrypted but should use a key management service in production
2. **JWT Tokens**: Change the JWT secret in production
3. **HTTPS**: Always use HTTPS in production
4. **Rate Limiting**: Implement rate limiting on API endpoints
5. **Input Validation**: All user inputs are validated and sanitized
6. **XSS Protection**: HTML content is sanitized using DOMPurify

## Performance Optimization

1. **Database Indexing**: Indexes on frequently queried columns
2. **Connection Pooling**: Reuse IMAP/SMTP connections
3. **Caching**: Cache recent emails locally
4. **Pagination**: Load conversations in batches
5. **Lazy Loading**: Load full conversation only when expanded

## Troubleshooting

### Common Issues

1. **IMAP Connection Failed**
   - Check firewall settings
   - Verify IMAP is enabled in email account
   - Try using app-specific passwords

2. **Database Errors**
   - Ensure migrations are run
   - Check file permissions for SQLite database

3. **Frontend Not Loading**
   - Clear browser cache
   - Check console for errors
   - Verify backend is running

## Contributing

### Frontend Development

The primary frontend is `frontend/app-working.html` - a standalone HTML file with embedded CSS and JavaScript.

**To modify the frontend:**
1. Edit `frontend/app-working.html` directly
2. Refresh your browser to see changes
3. No build step required!

**Optional TypeScript Development:**
If you prefer TypeScript for development, there's an alternative implementation in `frontend/src/`:
```bash
cd frontend
npm install
npm run dev
```

However, the standalone HTML version is the primary and recommended approach.

### Why Standalone HTML?

**Advantages:**
- ✅ Zero build time - edit and refresh
- ✅ No npm dependencies to manage
- ✅ No version conflicts or security vulnerabilities
- ✅ Instant startup
- ✅ Easy to understand and modify
- ✅ Works offline
- ✅ Tiny file size (~32KB)
- ✅ Deploy anywhere

1. Fork the repository
2. Create a feature branch
3. Commit your changes
4. Push to the branch
5. Create a Pull Request

## License

MIT License - See LICENSE file for details

## Roadmap

- [x] Conversation-based email interface
- [x] Reply, Reply All, Forward functionality
- [x] Quick reply from conversation view
- [x] Search and filtering
- [x] Keyboard shortcuts
- [ ] OAuth2 authentication (Gmail, Outlook)
- [ ] Email templates
- [ ] Multiple account support
- [ ] Dark mode theme
- [ ] Email encryption (PGP)
- [ ] Calendar integration

## Support

For issues and questions:
- 📖 Read the **[Quick Start Guide](QUICKSTART.md)**
- 🎨 Check the **[Frontend Architecture](FRONTEND.md)**
- 🐛 Create an issue on GitHub

## Philosophy

Frame Email Client embraces simplicity. We believe that modern web applications don't need complex build tools, hundreds of dependencies, or megabytes of JavaScript to be powerful and user-friendly.

**Simple. Fast. Effective.**