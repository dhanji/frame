"use client"

import {
  BadgeCheck,
  Bell,
  ChevronsUpDown,
  CreditCard,
  LogOut,
  Sparkles,
  Moon,
  Sun,
} from "lucide-react"

import {
  Avatar,
  AvatarFallback,
  AvatarImage,
} from "@/components/ui/avatar"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import {
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  useSidebar,
} from "@/components/ui/sidebar"
import { useTheme } from "@/contexts/theme-context"
import * as React from "react"

export function NavUser({
  user,
}: {
  user: {
    name: string
    email: string
    avatar: string
  }
}) {
  const { isMobile } = useSidebar()
  const { theme, setTheme } = useTheme()

  return (
    <SidebarMenu>
      <SidebarMenuItem>
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <SidebarMenuButton
              size="lg"
              className="data-[state=open]:bg-sidebar-accent data-[state=open]:text-sidebar-accent-foreground"
            >
              <Avatar className="h-8 w-8 rounded-lg">
                <AvatarImage src={user.avatar} alt={user.name} />
                <AvatarFallback className="rounded-lg">CN</AvatarFallback>
              </Avatar>
              <div className="grid flex-1 text-left text-sm leading-tight">
                <span className="truncate font-semibold">{user.name}</span>
                <div className="flex items-center gap-1">
                  <svg width="12" height="12" viewBox="0 0 15 15" fill="none" xmlns="http://www.w3.org/2000/svg">
                    <path d="M7.12988 14.2598C3.19922 14.2598 0 11.0605 0 7.12988C0 3.19238 3.19922 0 7.12988 0C11.0674 0 14.2598 3.19238 14.2598 7.12988C14.2598 11.0605 11.0674 14.2598 7.12988 14.2598ZM7.12305 11.0332C7.24609 11.0332 7.3418 10.9307 7.3418 10.8145V10.2061C8.49023 10.1445 9.50195 9.58398 9.50195 8.3877C9.50195 7.30762 8.6748 6.9043 7.65625 6.67188L7.3418 6.59668V4.84668C7.82715 4.90137 8.18945 5.14062 8.35352 5.57812C8.46289 5.81055 8.63379 5.94043 8.87988 5.94043C9.12598 5.94043 9.36523 5.79688 9.36523 5.50293C9.36523 4.65527 8.3877 4.05371 7.3418 3.97168V3.37012C7.3418 3.25391 7.24609 3.15137 7.12305 3.15137C7.00684 3.15137 6.9043 3.25391 6.9043 3.37012V3.97168C5.79688 4.04004 4.82617 4.66211 4.82617 5.74902C4.82617 6.81543 5.69434 7.24609 6.62402 7.45801L6.9043 7.51953V9.33105C6.35742 9.29004 5.98145 9.07812 5.7832 8.58594C5.66016 8.33984 5.5166 8.22363 5.27734 8.22363C4.99023 8.22363 4.77148 8.39453 4.77148 8.69531C4.77148 8.79785 4.79883 8.89355 4.83984 8.99609C5.09961 9.75488 5.96094 10.1582 6.9043 10.2129V10.8145C6.9043 10.9307 7.00684 11.0332 7.12305 11.0332ZM5.89258 5.65332C5.89258 5.16113 6.35742 4.9082 6.9043 4.84668V6.50098L6.84277 6.48047C6.28906 6.34375 5.89258 6.11816 5.89258 5.65332ZM7.3418 9.33789V7.61523L7.41699 7.63574C7.9707 7.76562 8.44238 7.95703 8.44238 8.49023C8.44238 9.06445 7.94336 9.29688 7.3418 9.33789Z" fill="#27C840"/>
                  </svg>
                  <span className="truncate text-xs text-emerald-500">$2413</span>
                </div>
              </div>
              <ChevronsUpDown className="ml-auto size-4" />
            </SidebarMenuButton>
          </DropdownMenuTrigger>
          <DropdownMenuContent
            className="w-[--radix-dropdown-menu-trigger-width] min-w-56 rounded-lg"
            side={isMobile ? "bottom" : "right"}
            align="end"
            sideOffset={4}
          >
            <DropdownMenuLabel className="p-0 font-normal">
              <div className="flex items-center gap-2 px-1 py-1.5 text-left text-sm">
                <Avatar className="h-8 w-8 rounded-lg">
                  <AvatarImage src={user.avatar} alt={user.name} />
                  <AvatarFallback className="rounded-lg">CN</AvatarFallback>
                </Avatar>
                <div className="grid flex-1 text-left text-sm leading-tight">
                  <span className="truncate font-semibold">{user.name}</span>
                  <div className="flex items-center gap-1">
                    <svg width="12" height="12" viewBox="0 0 15 15" fill="none" xmlns="http://www.w3.org/2000/svg">
                      <path d="M7.12988 14.2598C3.19922 14.2598 0 11.0605 0 7.12988C0 3.19238 3.19922 0 7.12988 0C11.0674 0 14.2598 3.19238 14.2598 7.12988C14.2598 11.0605 11.0674 14.2598 7.12988 14.2598ZM7.12305 11.0332C7.24609 11.0332 7.3418 10.9307 7.3418 10.8145V10.2061C8.49023 10.1445 9.50195 9.58398 9.50195 8.3877C9.50195 7.30762 8.6748 6.9043 7.65625 6.67188L7.3418 6.59668V4.84668C7.82715 4.90137 8.18945 5.14062 8.35352 5.57812C8.46289 5.81055 8.63379 5.94043 8.87988 5.94043C9.12598 5.94043 9.36523 5.79688 9.36523 5.50293C9.36523 4.65527 8.3877 4.05371 7.3418 3.97168V3.37012C7.3418 3.25391 7.24609 3.15137 7.12305 3.15137C7.00684 3.15137 6.9043 3.25391 6.9043 3.37012V3.97168C5.79688 4.04004 4.82617 4.66211 4.82617 5.74902C4.82617 6.81543 5.69434 7.24609 6.62402 7.45801L6.9043 7.51953V9.33105C6.35742 9.29004 5.98145 9.07812 5.7832 8.58594C5.66016 8.33984 5.5166 8.22363 5.27734 8.22363C4.99023 8.22363 4.77148 8.39453 4.77148 8.69531C4.77148 8.79785 4.79883 8.89355 4.83984 8.99609C5.09961 9.75488 5.96094 10.1582 6.9043 10.2129V10.8145C6.9043 10.9307 7.00684 11.0332 7.12305 11.0332ZM5.89258 5.65332C5.89258 5.16113 6.35742 4.9082 6.9043 4.84668V6.50098L6.84277 6.48047C6.28906 6.34375 5.89258 6.11816 5.89258 5.65332ZM7.3418 9.33789V7.61523L7.41699 7.63574C7.9707 7.76562 8.44238 7.95703 8.44238 8.49023C8.44238 9.06445 7.94336 9.29688 7.3418 9.33789Z" fill="#27C840"/>
                    </svg>
                    <span className="truncate text-xs text-emerald-500">$2413</span>
                  </div>
                </div>
              </div>
            </DropdownMenuLabel>
            <DropdownMenuSeparator />
            <DropdownMenuGroup>
              <DropdownMenuItem>
                <Sparkles />
                Upgrade to Pro
              </DropdownMenuItem>
            </DropdownMenuGroup>
            <DropdownMenuSeparator />
            <DropdownMenuGroup>
              <DropdownMenuItem onClick={() => setTheme("light")}>
                <Sun className="mr-2" />
                Light
                {theme === "light" && <span className="ml-auto">✓</span>}
              </DropdownMenuItem>
              <DropdownMenuItem onClick={() => setTheme("dark")}>
                <Moon className="mr-2" />
                Dark
                {theme === "dark" && <span className="ml-auto">✓</span>}
              </DropdownMenuItem>
            </DropdownMenuGroup>
            <DropdownMenuSeparator />
            <DropdownMenuGroup>
              <DropdownMenuItem>
                <BadgeCheck />
                Account
              </DropdownMenuItem>
              <DropdownMenuItem>
                <CreditCard />
                Billing
              </DropdownMenuItem>
              <DropdownMenuItem>
                <Bell />
                Notifications
              </DropdownMenuItem>
            </DropdownMenuGroup>
            <DropdownMenuSeparator />
            <DropdownMenuItem>
              <LogOut />
              Log out
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </SidebarMenuItem>
    </SidebarMenu>
  )
}
