# Frame Email Client - System Architecture & Data Flow

## System Architecture Diagram

```mermaid
graph TB
    subgraph "Client Layer"
        Browser[Web Browser]
        Mobile[Mobile Browser]
    end
    
    subgraph "Frontend Application"
        UI[TypeScript/HTML SPA]
        WS[WebSocket Client]
        API[API Client]
        Cache[LocalStorage Cache]
    end
    
    subgraph "Load Balancer"
        Nginx[Nginx Reverse Proxy]
    end
    
    subgraph "Backend Services"
        Web[Actix-Web Server]
        Auth[JWT Auth Service]
        WSServer[WebSocket Server]
        Queue[Task Queue]
    end
    
    subgraph "Data Layer"
        PG[(PostgreSQL)]
        Redis[(Redis Cache)]
        SQLite[(SQLite Dev)]
    end
    
    subgraph "Email Services"
        IMAP[IMAP Server]
        SMTP[SMTP Server]
    end
    
    subgraph "Background Jobs"
        Sync[Email Sync Worker]
        Filter[Filter Worker]
        Cleanup[Cleanup Worker]
    end
    
    Browser --> UI
    Mobile --> UI
    UI --> API
    UI --> WS
    API --> Nginx
    WS --> Nginx
    Nginx --> Web
    Nginx --> WSServer
    Web --> Auth
    Web --> PG
    Web --> Redis
    Web --> Queue
    Queue --> Sync
    Queue --> Filter
    Queue --> Cleanup
    Sync --> IMAP
    Web --> SMTP
    Sync --> PG
    Filter --> PG
    Cleanup --> PG
```

## Data Flow Diagrams

### 1. Email Sending Flow

```mermaid
sequenceDiagram
    participant U as User
    participant F as Frontend
    participant A as API
    participant V as Validator
    participant S as SMTP Service
    participant D as Database
    participant W as WebSocket
    
    U->>F: Compose Email
    F->>F: Rich Text Editing
    F->>A: POST /api/emails/send
    A->>V: Validate JWT Token
    V-->>A: Token Valid
    A->>V: Validate Email Data
    V-->>A: Data Valid
    A->>S: Send via SMTP
    S-->>A: Message ID
    A->>D: Save to Sent Folder
    D-->>A: Email Saved
    A-->>F: Success Response
    A->>W: Broadcast Update
    W-->>F: Real-time Update
    F->>U: Show Confirmation
```

### 2. Email Receiving Flow

```mermaid
sequenceDiagram
    participant I as IMAP Server
    participant B as Background Worker
    participant D as Database
    participant R as Redis Cache
    participant W as WebSocket
    participant F as Frontend
    
    loop Every 5 minutes
        B->>I: Check New Emails
        I-->>B: Email List
        B->>B: Parse Emails
        B->>B: Thread Grouping
        B->>D: Store Emails
        D-->>B: Stored
        B->>R: Update Cache
        R-->>B: Cached
        B->>W: Notify Clients
        W-->>F: New Email Alert
        F->>F: Update UI
    end
```

### 3. Conversation Threading Flow

```mermaid
flowchart LR
    E1[Email 1] --> T[Threading Engine]
    E2[Email 2] --> T
    E3[Email 3] --> T
    
    T --> H[Header Analysis]
    H --> R[References]
    H --> I[In-Reply-To]
    H --> S[Subject Matching]
    
    R --> G[Group by Thread ID]
    I --> G
    S --> G
    
    G --> C1[Conversation 1]
    G --> C2[Conversation 2]
    
    C1 --> P1[Preview: Last 3 msgs]
    C2 --> P2[Preview: Last 3 msgs]
    
    P1 --> UI[UI Display]
    P2 --> UI
```

### 4. Auto-Save Draft Flow

```mermaid
stateDiagram-v2
    [*] --> Typing
    Typing --> Debounce: User types
    Debounce --> Waiting: Start 2s timer
    Waiting --> SaveDraft: Timer expires
    Waiting --> Debounce: More typing
    SaveDraft --> API: Send to backend
    API --> Saved: Store in DB
    Saved --> Typing: Continue editing
    Saved --> [*]: User closes
```

## Component Architecture

### Backend Components

```
┌─────────────────────────────────────────────────────────┐
│                    Actix-Web Server                      │
├─────────────────────────────────────────────────────────┤
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │   Handlers   │  │  Middleware  │  │   Services   │  │
│  ├──────────────┤  ├──────────────┤  ├──────────────┤  │
│  │ • auth       │  │ • JWT Auth   │  │ • Email      │  │
│  │ • emails     │  │ • Rate Limit │  │ • IMAP       │  │
│  │ • folders    │  │ • CORS       │  │ • SMTP       │  │
│  │ • drafts     │  │ • Logger     │  │ • Cache      │  │
│  │ • search     │  │ • CSRF       │  │ • Background │  │
│  └──────────────┘  └──────────────┘  └──────────────┘  │
├─────────────────────────────────────────────────────────┤
│                     Data Layer                           │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │  PostgreSQL  │  │    Redis     │  │   SQLite     │  │
│  │  (Production)│  │   (Cache)    │  │    (Dev)     │  │
│  └──────────────┘  └──────────────┘  └──────────────┘  │
└─────────────────────────────────────────────────────────┘
```

### Frontend Components

```
┌─────────────────────────────────────────────────────────┐
│                  TypeScript Application                  │
├─────────────────────────────────────────────────────────┤
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │     Views    │  │  Components  │  │   Services   │  │
│  ├──────────────┤  ├──────────────┤  ├──────────────┤  │
│  │ • Login      │  │ • RichEditor │  │ • API Client │  │
│  │ • Inbox      │  │ • Thread     │  │ • WebSocket  │  │
│  │ • Compose    │  │ • Message    │  │ • Storage    │  │
│  │ • Settings   │  │ • Sidebar    │  │ • Auth       │  │
│  └──────────────┘  └──────────────┘  └──────────────┘  │
├─────────────────────────────────────────────────────────┤
│                    State Management                      │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │   User State │  │  Email State │  │   UI State   │  │
│  └──────────────┘  └──────────────┘  └──────────────┘  │
└─────────────────────────────────────────────────────────┘
```

## Database Schema

```mermaid
erDiagram
    USERS ||--o{ FOLDERS : has
    USERS ||--o{ EMAILS : owns
    USERS ||--o{ FILTERS : creates
    USERS ||--o{ SETTINGS : has
    FOLDERS ||--o{ EMAILS : contains
    EMAILS ||--o{ ATTACHMENTS : has
    EMAILS }o--|| THREADS : belongs_to
    
    USERS {
        uuid id PK
        string email UK
        string password_hash
        string imap_host
        int imap_port
        string smtp_host
        int smtp_port
        timestamp created_at
    }
    
    EMAILS {
        uuid id PK
        uuid user_id FK
        uuid folder_id FK
        string thread_id
        string subject
        string from_address
        text body_text
        text body_html
        boolean is_read
        boolean is_starred
        timestamp date
    }
    
    FOLDERS {
        uuid id PK
        uuid user_id FK
        string name
        string folder_type
        int unread_count
        int total_count
    }
    
    ATTACHMENTS {
        uuid id PK
        uuid email_id FK
        string filename
        string content_type
        int size
        blob content
    }
    
    THREADS {
        string id PK
        string subject
        timestamp last_activity
        int message_count
    }
```

## API Request/Response Flow

### Authentication Flow
```
1. User Login
   POST /api/auth/login
   ├── Validate credentials
   ├── Generate JWT token
   ├── Create session
   └── Return token + user info

2. Authenticated Request
   GET /api/conversations
   ├── Extract JWT from header
   ├── Validate token
   ├── Check rate limit
   ├── Process request
   └── Return data

3. Token Refresh
   POST /api/auth/refresh
   ├── Validate refresh token
   ├── Generate new access token
   └── Return new token
```

### Email Operations Flow
```
1. Send Email
   POST /api/emails/send
   ├── Validate recipients
   ├── Process attachments
   ├── Send via SMTP
   ├── Store in Sent folder
   └── Broadcast via WebSocket

2. Reply to Email
   POST /api/emails/{id}/reply
   ├── Fetch original email
   ├── Build reply headers
   ├── Send via SMTP
   ├── Update thread
   └── Notify participants

3. Mark as Read
   PUT /api/emails/{id}/read
   ├── Update database
   ├── Update folder counts
   ├── Invalidate cache
   └── Send WebSocket update
```

## Performance Optimization Strategy

### Caching Layers
```
1. Browser Cache
   ├── Static assets (1 year)
   ├── API responses (5 minutes)
   └── User preferences (persistent)

2. Redis Cache
   ├── Conversation lists (5 minutes)
   ├── Folder counts (30 seconds)
   ├── User sessions (24 hours)
   └── Search results (10 minutes)

3. Database Query Cache
   ├── Prepared statements
   ├── Connection pooling
   └── Index optimization
```

### Load Distribution
```
1. Horizontal Scaling
   ├── Multiple backend instances
   ├── Load balancer (Nginx)
   ├── Sticky sessions for WebSocket
   └── Shared Redis cache

2. Background Processing
   ├── Email sync queue
   ├── Filter processing queue
   ├── Attachment processing
   └── Cleanup tasks
```

## Security Architecture

### Defense in Depth
```
1. Network Layer
   ├── HTTPS only
   ├── Firewall rules
   ├── DDoS protection
   └── Rate limiting

2. Application Layer
   ├── JWT authentication
   ├── CSRF tokens
   ├── Input validation
   └── SQL injection prevention

3. Data Layer
   ├── Encryption at rest
   ├── Encrypted credentials
   ├── Secure backups
   └── Audit logging
```

## Monitoring & Observability

### Metrics Collection
```
1. Application Metrics
   ├── Request rate
   ├── Response time
   ├── Error rate
   └── Active users

2. System Metrics
   ├── CPU usage
   ├── Memory usage
   ├── Disk I/O
   └── Network traffic

3. Business Metrics
   ├── Emails sent/received
   ├── User engagement
   ├── Search queries
   └── Feature usage
```

### Logging Strategy
```
1. Application Logs
   ├── Error logs
   ├── Access logs
   ├── Audit logs
   └── Debug logs

2. Log Aggregation
   ├── Centralized logging (Loki)
   ├── Log parsing
   ├── Alert rules
   └── Retention policy
```

## Deployment Pipeline

```mermaid
graph LR
    Dev[Development] --> Test[Testing]
    Test --> Stage[Staging]
    Stage --> Prod[Production]
    
    Test --> UT[Unit Tests]
    Test --> IT[Integration Tests]
    Test --> E2E[E2E Tests]
    
    Stage --> PT[Performance Tests]
    Stage --> ST[Security Tests]
    
    Prod --> Mon[Monitoring]
    Prod --> Backup[Backups]
```

## Disaster Recovery

### Backup Strategy
```
1. Database Backups
   ├── Daily full backup
   ├── Hourly incremental
   ├── Off-site storage
   └── 30-day retention

2. Recovery Procedures
   ├── RTO: 1 hour
   ├── RPO: 1 hour
   ├── Automated restore
   └── Failover testing
```

---

This architecture ensures scalability, reliability, and performance while meeting all specified requirements for the Frame Email Client.