import { AppSidebar } from "@/components/app-sidebar"
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from "@/components/ui/breadcrumb"
import { Separator } from "@/components/ui/separator"
import {
  SidebarInset,
  SidebarProvider,
  SidebarTrigger,
} from "@/components/ui/sidebar"
import { Inbox, Reply, ReplyAll, Forward, MoreVertical, Search, ChevronDown, MessageCircle } from "lucide-react"
import * as React from "react"

// Sample email list data
const emailListData = [
  {
    id: 1,
    subject: "Project Update - Q4 2024",
    preview: "The Q4 project is progressing well. We have completed 75% of the planned fea...",
    sender: "Goose",
    timestamp: "3m ago",
    unread: true,
    section: "TODAY",
  },
  {
    id: 2,
    subject: "Daily Brief",
    preview: "Here's an overview of your meeting today with Gleisson Cabral from Mercado Bit...",
    sender: "Goose",
    timestamp: "2h ago",
    unread: false,
    badge: "12",
    section: "TODAY",
  },
  {
    id: 3,
    subject: "Port Numbers",
    preview: "Can someone help explain how to set up port numbers?",
    sender: "V33",
    timestamp: "May 18, 2020 9:00 PM",
    unread: false,
    badge: "12",
    section: "YESTERDAY",
  },
  {
    id: 4,
    subject: "Starting out with Expressions",
    preview: "Question #5 on the starting out with expressions lesson?",
    sender: "Francis Divo",
    timestamp: "May 18 | Intro to Ex...",
    unread: false,
    badge: "12",
    section: "YESTERDAY",
  },
  {
    id: 5,
    subject: "Port Numbers",
    preview: "Can someone help explain how to set up port numbers?",
    sender: "Kevin Gordan",
    timestamp: "May 18, 2020 5:00 PM | Intro to...",
    unread: false,
    badge: "12",
    section: "YESTERDAY",
  },
]

export default function App() {
  const [selectedEmailId, setSelectedEmailId] = React.useState(1)
  const [replyingTo, setReplyingTo] = React.useState<number | null>(null)
  const [replyText, setReplyText] = React.useState("")

  return (
    <SidebarProvider
      style={
        {
          "--sidebar-width": "260px",
        } as React.CSSProperties
      }
    >
      <AppSidebar />
      <SidebarInset>
        {/* Two Column Layout: Email List + Email Detail */}
        <div className="flex flex-1 overflow-hidden">
          {/* Left Column - Email List Preview */}
          <div className="w-80 border-r overflow-y-auto flex flex-col" style={{ backgroundColor: "var(--background)" }}>
            {/* Inbox Header */}
            <div className="px-4 py-3 flex items-center gap-3">
              <Inbox className="w-5 h-5 text-slate-900 dark:text-slate-100 flex-shrink-0" />
              <h2 className="text-lg font-semibold text-slate-900 dark:text-slate-100">Inbox</h2>
            </div>
            
            {/* Search Bar */}
            <div className="px-4 py-3">
              <div className="flex items-center gap-2 rounded-lg px-3 py-2" style={{ backgroundColor: "hsl(var(--sidebar-accent))" }}>
                <Search className="w-4 h-4 text-slate-900 dark:text-slate-100 flex-shrink-0" />
                <input
                  type="text"
                  placeholder="Search"
                  className="flex-1 bg-transparent text-sm outline-none text-slate-900 dark:text-slate-100 placeholder-slate-500 dark:placeholder-slate-400"
                />
              </div>
            </div>

            {/* Email List */}
            <div className="flex-1 overflow-y-auto">
              {emailListData.reduce((acc, email) => {
                const lastSection = acc[acc.length - 1]?.section;
                
                // Add section header if new section
                if (lastSection !== email.section) {
                  acc.push({
                    type: "section",
                    section: email.section,
                  } as any);
                }
                
                acc.push(email);
                return acc;
              }, [] as any[]).map((item) => 
                item.type === "section" ? (
                  <div key={`section-${item.section}`} className="px-4 py-2 text-xs font-semibold text-slate-500 dark:text-slate-400 uppercase tracking-wide">
                    {item.section}
                  </div>
                ) : (
                  <div
                    key={item.id}
                    onClick={() => setSelectedEmailId(item.id)}
                    className={`mx-1 px-3 py-2.5 cursor-pointer transition-colors ${
                      selectedEmailId === item.id ? "rounded" : ""
                    }`}
                    style={selectedEmailId === item.id ? { backgroundColor: "hsl(var(--sidebar-accent))" } : {}}
                  >
                    <div className="flex items-start gap-3">
                      {/* Unread Indicator */}
                      {item.unread && (
                        <div className="w-2 h-2 rounded-full bg-blue-600 dark:bg-green-500 mt-2.5 flex-shrink-0" />
                      )}
                      {!item.unread && (
                        <div className="w-2 h-2 rounded-sm bg-zinc-400 dark:bg-zinc-600 mt-2.5 flex-shrink-0" />
                      )}
                      
                      <div className="flex-1 min-w-0 space-y-1">
                        <div className="flex items-start justify-between gap-2">
                          <h3 className="text-sm font-semibold text-slate-900 dark:text-slate-100 line-clamp-1">
                            {item.subject}
                          </h3>
                          {item.badge && (
                            <span className="text-xs font-semibold text-slate-500 dark:text-slate-400 flex-shrink-0">
                              {item.badge}
                            </span>
                          )}
                        </div>
                        <p className="text-sm text-slate-600 dark:text-slate-400 line-clamp-2">
                          {item.preview}
                        </p>
                        <p className="text-xs text-slate-500 dark:text-slate-500">
                          {item.sender} · {item.timestamp}
                        </p>
                      </div>
                    </div>
                  </div>
                )
              )}
            </div>
          </div>

          {/* Right Column - Email Detail */}
          <div className="flex-1 overflow-y-auto bg-white" style={{ backgroundColor: 'var(--background)' }}>
            {/* Header area aligned with left panel */}
            <div className="px-6 pt-3 pb-6">
            {(() => {
              const selectedEmail = emailListData.find(e => e.id === selectedEmailId);
              if (!selectedEmail) return null;
              
              const emailDetails: Record<number, { subject: string; sender: string; time: string; body: string }> = {
  1: {
    subject: "Project Update - Q4 2024",
    sender: "Goose",
    time: "3m ago",
    body: `Hi team,

Quick status: We're ~75% through the Q4 plan and tracking the December release. Velocity is steady and the burndown has stayed within tolerance for three weeks.

Highlights this week
- API integration completed; endpoints validated and monitored (P95 < 200ms)
- Database schema finalized; migrations and backups tested
- Frontend performance pass: -23% JS, first load ~2.3s on baseline device

What's next
- Nov 5–12: moderated user testing (50 participants)
- Nov 13–19: performance benchmarking and load testing
- Nov 20–26: security review and pen‑test preparation

Notes
- Backend: 8  ·  Frontend: 6  ·  QA: 4  ·  DevOps: 2  ·  PM: 1
- Settings flow design handed off; copy ready for review

Risks & watchlist
- Third‑party API flakiness during peak hours (mitigated with retries and circuit breaker)
- Holiday availability — schedules mapped, no critical gaps expected

If anything looks off or you’d like deeper numbers, ping me and I’ll share the dashboards.

Thanks,
Goose`
  },
  2: {
    subject: "Daily Brief",
    sender: "Goose",
    time: "2h ago",
    body: `Good afternoon,

Here is a brief summary of today's key activities and decisions:

• Meeting with Mercado Bit (30 min)
  - Topics: onboarding pipeline, data export cadence, error triage
  - Decisions: move exports to 6AM UTC; add alerting for 5xx spikes
  - Follow-ups: schedule integration demo Friday; share dashboard link

• Inbox Triage
  - 156 emails processed; 42 tagged 'important' based on sender + keywords
  - 8 messages flagged as spam; models updated with feedback
  - 0 messages remaining in 'Needs Review'

• Recommendations
  - Create 'Vendors' folder (7 threads auto-detected)
  - Archive newsletters older than 60 days to reduce noise
  - Enable smart templates for common replies (ETA/receipt/thanks)

Have a great rest of your day!`
  },
  3: {
    subject: "Port Numbers",
    sender: "V33",
    time: "May 18, 2020 9:00 PM",
    body: `Hey team,

I'm struggling to get our dev environment reachable from the local network. Could someone confirm the correct port mapping and firewall rules?

Environment: Docker + Nginx reverse proxy
Suggested config:
  - App: 8080 -> 80 (container)
  - API: 8081 -> 3000 (container)
  - Websocket: 8082 -> 6001 (container)

Checklist:
  1) lsof -i :8080 (verify listener)
  2) ufw allow 8080/tcp (or macOS pfctl rule)
  3) curl -I http://localhost:8080 (expect 200/301)
  4) Update CORS to include http://localhost:8080

Thanks in advance!`
  },
  4: {
    subject: "Starting out with Expressions",
    sender: "Francis Divo",
    time: "May 18 | Intro to Ex",
    body: `Question on expression syntax:

Given an input array, I want to transform values and filter nulls in one pass. Is the following idiomatic?

Steps:
  - map(value => value?.trim())
  - filter(v => v && v.length > 0)
  - reduce((acc, v) => acc + v.length, 0)

Edge cases considered: unicode whitespace, RTL markers, zero-width joiners.

If you have a simpler approach or perf tips (esp. for 50k+ items), please share.`
  },
  5: {
    subject: "Port Numbers",
    sender: "Kevin Gordan",
    time: "May 18, 2020 5:00 PM",
    body: `Following up on ports for prod:

Reverse proxy: Nginx
  - 80 -> app (HTTP) redirects to 443
  - 443 -> app upstream (TLS 1.3 preferred)
  - websockets upgrade for /ws maintained

Security:
  - Only 80/443 open externally
  - Healthcheck on /health (200)
  - Rate limiting: 100 rpm per IP

If this looks good, I'll open the change request and schedule maintenance.`
  }
};

              const detail = emailDetails[selectedEmail.id];
              
              return (
                <div className="space-y-4">
                  <h1 className="text-xl font-semibold text-slate-900 dark:text-slate-100">{detail.subject}</h1>
                  <div className="flex items-center justify-between">
                    <div className="flex items-center gap-3">
                      <div className="w-10 h-10 rounded-xl bg-zinc-800 dark:bg-zinc-700 flex items-center justify-center text-white font-semibold"><Inbox className="w-5 h-5" /></div>
                      <div>
                        <div className="text-sm font-medium text-slate-900 dark:text-slate-100">{detail.sender}</div>
                        <div className="text-sm text-slate-500 dark:text-slate-400">{detail.time}</div>
                      </div>
                    </div>
                    <div className="flex items-center gap-2">
                      <button className="p-2 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-lg transition-colors text-slate-900 dark:text-slate-100"><Reply className="w-5 h-5" /></button>
                      <button className="p-2 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-lg transition-colors text-slate-900 dark:text-slate-100"><ReplyAll className="w-5 h-5" /></button>
                      <button className="p-2 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-lg transition-colors text-slate-900 dark:text-slate-100"><Forward className="w-5 h-5" /></button>
                      <Separator orientation="vertical" className="h-6" />
                      <button className="p-2 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-lg transition-colors text-slate-900 dark:text-slate-100"><MoreVertical className="w-5 h-5" /></button>
                    </div>
                  </div>
                  <div className="prose prose-sm max-w-none mt-6">
                    <p className="text-slate-900 dark:text-slate-100 leading-relaxed whitespace-pre-wrap text-sm">{detail.body}</p>
                  </div>
                  {/* Reply Section */}
                  {replyingTo === selectedEmailId ? (
                    <div className="border rounded-lg p-4 bg-slate-50 mt-6" style={{ backgroundColor: 'var(--card)' }}>
                      <textarea className="w-full h-32 p-3 text-sm text-slate-900 dark:text-slate-100 bg-transparent border border-slate-300 dark:border-slate-700 rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500 dark:focus:ring-blue-400" placeholder="Start typing your reply..." value={replyText} onChange={(e) => setReplyText(e.target.value)} />
                      <div className="flex items-start gap-4 mt-4">
                        <button className="h-8 px-4 bg-blue-600 hover:bg-blue-700 dark:bg-blue-600 dark:hover:bg-blue-700 text-white rounded-full flex items-center gap-2 text-sm font-medium transition-colors"><Reply className="w-4 h-4" />Reply</button>
                        <button onClick={() => setReplyingTo(null)} className="h-8 px-4 bg-slate-300 hover:bg-slate-400 dark:bg-slate-700 dark:hover:bg-slate-600 text-slate-900 dark:text-slate-100 rounded-full text-sm font-medium transition-colors">Cancel</button>
                      </div>
                    </div>
                  ) : (
                    <div className="flex items-center gap-2 mt-6">
                      <button onClick={() => setReplyingTo(selectedEmailId)} className="h-8 px-4 bg-blue-600 hover:bg-blue-700 dark:bg-blue-600 dark:hover:bg-blue-700 text-white rounded-full flex items-center gap-2 text-sm font-medium transition-colors"><Reply className="w-4 h-4" />Reply</button>
                      <button className="h-8 px-4 text-slate-900 dark:text-slate-100 rounded-full flex items-center gap-2 text-sm font-medium transition-colors" style={{ backgroundColor: "hsl(var(--sidebar-accent))" }}><ReplyAll className="w-4 h-4" />Reply All</button>
                      <button className="h-8 px-4 text-slate-900 dark:text-slate-100 rounded-full flex items-center gap-2 text-sm font-medium transition-colors" style={{ backgroundColor: "hsl(var(--sidebar-accent))" }}><Forward className="w-4 h-4" />Forward</button>
                      <button className="h-8 px-4 text-slate-900 dark:text-slate-100 rounded-full flex items-center gap-2 text-sm font-medium transition-colors" style={{ backgroundColor: "hsl(var(--sidebar-accent))" }}><MessageCircle className="w-4 h-4" />Chat</button>
                    </div>
                  )}
                </div>
              );
            })()}
            </div>
          </div>

        </div>
      </SidebarInset>
    </SidebarProvider>
  )
}
