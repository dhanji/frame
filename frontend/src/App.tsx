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
import { Inbox, Reply, ReplyAll, Forward, MoreVertical, Search, ChevronDown } from "lucide-react"
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
          <div className="w-80 border-r bg-white overflow-y-auto flex flex-col" style={{ backgroundColor: 'var(--background)' }}>
            {/* Inbox Header */}
            <div className="px-4 py-3 flex items-center gap-3 border-b">
              <Inbox className="w-5 h-5 text-slate-900 dark:text-slate-100 flex-shrink-0" />
              <h2 className="text-lg font-semibold text-slate-900 dark:text-slate-100">Inbox</h2>
            </div>
            
            {/* Search Bar */}
            <div className="px-4 py-3 border-b">
              <div className="flex items-center gap-2 bg-gray-100 dark:bg-slate-700 rounded-lg px-3 py-2">
                <Search className="w-4 h-4 text-slate-900 dark:text-slate-100 flex-shrink-0" />
                <input
                  type="text"
                  placeholder="Search"
                  className="flex-1 bg-transparent text-sm outline-none text-slate-900 dark:text-slate-100 placeholder-slate-500 dark:placeholder-slate-400"
                />
                <ChevronDown className="w-4 h-4 text-slate-900 dark:text-slate-100 flex-shrink-0" />
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
                  <div key={`section-${item.section}`} className="px-4 py-2 text-xs font-semibold text-slate-500 dark:text-slate-400 uppercase tracking-wide border-t">
                    {item.section}
                  </div>
                ) : (
                  <div
                    key={item.id}
                    onClick={() => setSelectedEmailId(item.id)}
                    className={`px-4 py-3 border-b cursor-pointer transition-colors ${
                      selectedEmailId === item.id ? "border-l-4 rounded" : ""
                    }`}
                    style={selectedEmailId === item.id ? { 
                      backgroundColor: 'hsl(var(--primary) / 0.15)', 
                      borderLeftColor: 'var(--primary)'
                    } : {}}
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
            {/* Email Title as Header */}
            <div className="space-y-4">
              <h1 className="text-xl font-semibold text-slate-900 dark:text-slate-100">
                Project Update - Q4 2024
              </h1>
              
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-3">
                  <div className="w-10 h-10 rounded-xl bg-zinc-800 dark:bg-zinc-700 flex items-center justify-center text-white font-semibold">
                    <Inbox className="w-5 h-5" />
                  </div>
                  <div>
                    <div className="text-sm font-medium text-slate-900 dark:text-slate-100">Goose Mail</div>
                    <div className="text-sm text-slate-500 dark:text-slate-400">2h ago</div>
                  </div>
                </div>
                
                <div className="flex items-center gap-2">
                  <button className="p-2 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-lg transition-colors text-slate-900 dark:text-slate-100">
                    <Reply className="w-5 h-5" />
                  </button>
                  <button className="p-2 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-lg transition-colors text-slate-900 dark:text-slate-100">
                    <ReplyAll className="w-5 h-5" />
                  </button>
                  <button className="p-2 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-lg transition-colors text-slate-900 dark:text-slate-100">
                    <Forward className="w-5 h-5" />
                  </button>
                  <Separator orientation="vertical" className="h-6" />
                  <button className="p-2 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-lg transition-colors text-slate-900 dark:text-slate-100">
                    <MoreVertical className="w-5 h-5" />
                  </button>
                </div>
              </div>
            </div>

            {/* Email Body */}
            <div className="prose prose-sm max-w-none">
              <p className="text-slate-900 dark:text-slate-100 leading-relaxed">
                The Q4 project is progressing well. We have completed approximately 75% of the planned features and are now transitioning from foundational development into integration and testing of the new framework — Goose Control.
              </p>
              
              <p className="text-slate-900 dark:text-slate-100 leading-relaxed">
                Goose Control is a next-generation orchestration layer for managing distributed AI agents and their environments. The system introduces a unified way to deploy, observe, and modify agent behavior across both human and machine networks. Instead of building isolated agent systems, Goose Control focuses on interoperability — allowing multiple agents, tools, and users to coexist and collaborate under a consistent governance and permissions structure.
              </p>

              <p className="text-slate-900 dark:text-slate-100 font-medium mt-6 leading-relaxed">
                Over the past quarter, our focus has been on three core areas:
              </p>

              <div className="space-y-4 mt-4 text-slate-900 dark:text-slate-100 text-sm leading-relaxed">
                <ol className="list-decimal list-inside space-y-3">
                  <li>
                    <span className="font-semibold">Core Engine</span>
                    <p className="ml-6 -mt-2 text-slate-900 dark:text-slate-100">The foundation of Goose Control now supports real-time state management, event-driven task routing, and hierarchical delegation between agents. We've implemented a modular "control loop" architecture that lets each agent act autonomously while remaining responsive to higher-level coordination rules. This enables a swarm-like behavior pattern where multiple agents can contribute to a shared objective without central bottlenecks.</p>
                  </li>

                  <li>
                    <span className="font-semibold">Interface & Protocol Layer</span>
                    <p className="ml-6 -mt-2 text-slate-900 dark:text-slate-100">We've developed a flexible command schema and message routing system that allows Goose Control to operate seamlessly across different environments — from web dashboards and CLIs to chat-based interfaces like Telegram or Slack. This abstraction layer allows agents and users to communicate through a common protocol, enabling both human-readable commands and machine-executable instructions.</p>
                  </li>

                  <li>
                    <span className="font-semibold">Security & Permission System</span>
                    <p className="ml-6 -mt-2 text-slate-900 dark:text-slate-100">The framework now includes a tiered permission system with sandboxed execution and auditable event logs. This ensures that each agent operates within defined limits while maintaining transparency of all actions. The groundwork for role-based access control (RBAC) and identity verification is also in place, paving the way for secure agent collaboration within shared environments.</p>
                  </li>
                </ol>
              </div>

              <p className="text-slate-900 dark:text-slate-100 mt-6 leading-relaxed">
                In parallel, the team has been working on agent introspection tools — lightweight modules that allow developers to visualize and debug agent thought processes in real time — as well as adaptive UI components that dynamically respond to agent activity. These tools will make Goose Control not only a coordination layer but also an observability platform for multi-agent systems.
              </p>
            </div>

            {/* Reply Box */}
            <div className="border rounded-lg p-4 bg-slate-50" style={{ backgroundColor: 'var(--card)' }}>
              <p className="text-sm text-slate-900 dark:text-slate-100">
                Looking great! For next time please make sure you include what each team member have been working on so that we are clear on who's done what.
              </p>
            </div>

            {/* Reply Button */}
            <div className="flex items-start gap-4">
              <button className="h-8 px-4 bg-zinc-800 dark:bg-zinc-700 hover:bg-zinc-700 dark:hover:bg-zinc-600 text-white rounded-full flex items-center gap-2 text-sm font-medium transition-colors">
                <Reply className="w-4 h-4" />
                Reply
              </button>
            </div>
            </div>
          </div>
        </div>
      </SidebarInset>
    </SidebarProvider>
  )
}
