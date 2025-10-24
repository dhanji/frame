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
import { Mail, Reply, ReplyAll, Forward, MoreVertical } from "lucide-react"

export default function App() {
  return (
    <SidebarProvider
      style={
        {
          "--sidebar-width": "350px",
        } as React.CSSProperties
      }
    >
      <AppSidebar />
      <SidebarInset>
        <header className="bg-background sticky top-0 flex shrink-0 items-center gap-2 border-b p-4">
          <SidebarTrigger className="-ml-1" />
          <Separator
            orientation="vertical"
            className="mr-2 data-[orientation=vertical]:h-4"
          />
          <Breadcrumb>
            <BreadcrumbList>
              <BreadcrumbItem className="hidden md:block">
                <BreadcrumbLink href="#">All Inboxes</BreadcrumbLink>
              </BreadcrumbItem>
              <BreadcrumbSeparator className="hidden md:block" />
              <BreadcrumbItem>
                <BreadcrumbPage>Inbox</BreadcrumbPage>
              </BreadcrumbItem>
            </BreadcrumbList>
          </Breadcrumb>
        </header>
        
        {/* Email Content Area */}
        <div className="flex-1 overflow-auto">
          <div className="p-6 space-y-6">
            {/* Email Header */}
            <div className="space-y-4">
              <h1 className="text-xl font-semibold text-slate-900">
                Project Update - Q4 2024
              </h1>
              
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-3">
                  <div className="w-10 h-10 rounded-xl bg-zinc-800 flex items-center justify-center text-white font-semibold">
                    <Mail className="w-5 h-5" />
                  </div>
                  <div>
                    <div className="text-sm font-medium text-slate-900">Goose Mail</div>
                    <div className="text-sm text-muted-foreground">2h ago</div>
                  </div>
                </div>
                
                <div className="flex items-center gap-2">
                  <button className="p-2 hover:bg-accent rounded-lg transition-colors">
                    <Reply className="w-5 h-5" />
                  </button>
                  <button className="p-2 hover:bg-accent rounded-lg transition-colors">
                    <ReplyAll className="w-5 h-5" />
                  </button>
                  <button className="p-2 hover:bg-accent rounded-lg transition-colors">
                    <Forward className="w-5 h-5" />
                  </button>
                  <Separator orientation="vertical" className="h-6" />
                  <button className="p-2 hover:bg-accent rounded-lg transition-colors">
                    <MoreVertical className="w-5 h-5" />
                  </button>
                </div>
              </div>
            </div>

            {/* Email Body */}
            <div className="prose prose-sm max-w-none">
              <p className="text-slate-900">
                The Q4 project is progressing well. We have completed approximately 75% of the planned features and are now transitioning from foundational development into integration and testing of the new framework — Goose Control.
              </p>
              
              <p className="text-slate-900">
                Goose Control is a next-generation orchestration layer for managing distributed AI agents and their environments. The system introduces a unified way to deploy, observe, and modify agent behavior across both human and machine networks. Instead of building isolated agent systems, Goose Control focuses on interoperability — allowing multiple agents, tools, and users to coexist and collaborate under a consistent governance and permissions structure.
              </p>

              <p className="text-slate-900 font-medium mt-6">
                Over the past quarter, our focus has been on three core areas:
              </p>

              <div className="space-y-4 mt-4">
                <div>
                  <h3 className="text-slate-900 font-semibold">1. Core Engine</h3>
                  <p className="text-slate-900 mt-2">
                    The foundation of Goose Control now supports real-time state management, event-driven task routing, and hierarchical delegation between agents. We've implemented a modular "control loop" architecture that lets each agent act autonomously while remaining responsive to higher-level coordination rules. This enables a swarm-like behavior pattern where multiple agents can contribute to a shared objective without central bottlenecks.
                  </p>
                </div>

                <div>
                  <h3 className="text-slate-900 font-semibold">2. Interface & Protocol Layer</h3>
                  <p className="text-slate-900 mt-2">
                    We've developed a flexible command schema and message routing system that allows Goose Control to operate seamlessly across different environments — from web dashboards and CLIs to chat-based interfaces like Telegram or Slack. This abstraction layer allows agents and users to communicate through a common protocol, enabling both human-readable commands and machine-executable instructions.
                  </p>
                </div>

                <div>
                  <h3 className="text-slate-900 font-semibold">3. Security & Permission System</h3>
                  <p className="text-slate-900 mt-2">
                    The framework now includes a tiered permission system with sandboxed execution and auditable event logs. This ensures that each agent operates within defined limits while maintaining transparency of all actions. The groundwork for role-based access control (RBAC) and identity verification is also in place, paving the way for secure agent collaboration within shared environments.
                  </p>
                </div>
              </div>

              <p className="text-slate-900 mt-6">
                In parallel, the team has been working on agent introspection tools — lightweight modules that allow developers to visualize and debug agent thought processes in real time — as well as adaptive UI components that dynamically respond to agent activity. These tools will make Goose Control not only a coordination layer but also an observability platform for multi-agent systems.
              </p>
            </div>

            {/* Reply Box */}
            <div className="border rounded-lg p-4">
              <p className="text-sm text-slate-900">
                Looking great! For next time please make sure you include what each team member have been working on so that we are clear on who's done what.
              </p>
            </div>

            {/* Reply Button */}
            <div className="flex items-start gap-4">
              <button className="h-8 px-4 bg-zinc-800 hover:bg-zinc-700 text-white rounded-full flex items-center gap-2 text-sm font-medium transition-colors">
                <Reply className="w-4 h-4" />
                Reply
              </button>
            </div>
          </div>
        </div>
      </SidebarInset>
    </SidebarProvider>
  )
}
