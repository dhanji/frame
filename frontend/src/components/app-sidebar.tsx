import * as React from "react"
import {
  Command,
  Mail,
  MessageSquare,
  Calendar,
  CheckSquare2,
  Settings2,
} from "lucide-react"

import { NavMain } from "@/components/nav-main"
import { NavProjects } from "@/components/nav-projects"
import { NavUser } from "@/components/nav-user"
import { TeamSwitcher } from "@/components/team-switcher"
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarHeader,
  SidebarRail,
} from "@/components/ui/sidebar"

// This is sample data.
const data = {
  user: {
    name: "dhanji",
    email: "user@goosepatrol.com",
    avatar: "/avatars/shadcn.jpg",
  },
  teams: [
    {
      name: "Goose Patrol",
      logo: Command,
      plan: "Enterprise",
    },
  ],
  navMain: [
    {
      title: "Inbox",
      url: "#",
      icon: Mail,
      isActive: true,
      items: [
        {
          title: "Inbox",
          url: "#",
        },
        {
          title: "Drafts",
          url: "#",
        },
        {
          title: "Sent",
          url: "#",
        },
        {
          title: "Junk",
          url: "#",
        },
        {
          title: "Trash",
          url: "#",
        },
      ],
    },
    {
      title: "Chat",
      url: "#",
      icon: MessageSquare,
      items: [
        {
          title: "Direct Messages",
          url: "#",
        },
        {
          title: "Channels",
          url: "#",
        },
        {
          title: "Archived",
          url: "#",
        },
      ],
    },
    {
      title: "Calendar",
      url: "#",
      icon: Calendar,
      items: [
        {
          title: "My Calendar",
          url: "#",
        },
        {
          title: "Team Calendar",
          url: "#",
        },
        {
          title: "Shared",
          url: "#",
        },
      ],
    },
    {
      title: "Tasks",
      url: "#",
      icon: CheckSquare2,
      items: [
        {
          title: "My Tasks",
          url: "#",
        },
        {
          title: "Assigned to Me",
          url: "#",
        },
        {
          title: "Completed",
          url: "#",
        },
      ],
    },
  ],
  projects: [
    {
      name: "Settings",
      url: "#",
      icon: Settings2,
    },
  ],
}

export function AppSidebar({ ...props }: React.ComponentProps<typeof Sidebar>) {
  return (
    <Sidebar collapsible="icon" {...props}>
      <SidebarHeader>
        <TeamSwitcher teams={data.teams} />
      </SidebarHeader>
      <SidebarContent>
        <NavMain items={data.navMain} />
        <NavProjects projects={data.projects} />
      </SidebarContent>
      <SidebarFooter>
        <NavUser user={data.user} />
      </SidebarFooter>
      <SidebarRail />
    </Sidebar>
  )
}
