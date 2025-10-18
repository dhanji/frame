# Frame Email Client

A modern email client with a conversation-based interface, similar to Facebook's feed design. Built with Rust (backend) and TypeScript/HTML (frontend).

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

### Frontend (TypeScript/HTML)
- **Build Tool**: Vite
- **Type Safety**: Full TypeScript implementation
- **API Client**: Axios for HTTP requests
- **Date Handling**: date-fns
- **HTML Sanitization**: DOMPurify

## Installation & Setup

### Prerequisites
- Rust (latest stable version)
- Node.js (v16 or higher)
- npm or yarn

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

### Frontend Setup

1. Navigate to the frontend directory:
```bash
cd email-client/frontend
```

2. Install dependencies:
```bash
npm install
```

3. Start the development server:
```bash
npm run dev
```

The frontend will be available at `http://localhost:3000`

### Production Build

#### Backend
```bash
cd email-client/backend
cargo build --release
./target/release/email-server
```

#### Frontend
```bash
cd email-client/frontend
npm run build
# Serve the dist folder with any static file server
```

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

### Running Tests

#### Backend Tests
```bash
cd email-client/backend
cargo test
```

#### Frontend Tests
```bash
cd email-client/frontend
npm test
```

### Database Migrations

Create a new migration:
```bash
sqlx migrate add <migration_name>
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
│   ├── src/
│   │   ├── api/         # API client
│   │   ├── models/      # TypeScript types
│   │   ├── utils/       # Helper functions
│   │   ├── styles/      # CSS files
│   │   └── main.ts      # Application entry point
│   ├── index.html       # Main HTML file
│   └── package.json     # Node dependencies
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

1. Fork the repository
2. Create a feature branch
3. Commit your changes
4. Push to the branch
5. Create a Pull Request

## License

MIT License - See LICENSE file for details

## Roadmap

- [ ] Add OAuth2 authentication
- [ ] Implement push notifications
- [ ] Add email templates
- [ ] Support for multiple accounts
- [ ] Calendar integration
- [ ] Contact management
- [ ] Email encryption (PGP)
- [ ] Advanced filtering rules
- [ ] Keyboard shortcuts
- [ ] Dark mode theme

## Support

For issues and questions, please create an issue in the GitHub repository.