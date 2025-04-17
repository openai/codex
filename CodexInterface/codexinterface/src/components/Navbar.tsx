import React from 'react'
import { Terminal, Settings, Brain, Mic, FileText } from 'lucide-react'
import { cn } from '../lib/utils'

interface NavItemProps {
  icon: React.ReactNode;
  label: string;
  active: boolean;
  onClick: () => void;
}

const NavItem: React.FC<NavItemProps> = ({ icon, label, active, onClick }) => {
  return (
    <div 
      className={cn(
        "flex items-center gap-2 px-4 py-2 cursor-pointer rounded-md transition-colors",
        active 
          ? "bg-primary/20 text-primary border-r-2 border-primary" 
          : "text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800/50"
      )}
      onClick={onClick}
    >
      <div className="w-5 h-5 flex items-center justify-center">
        {icon}
      </div>
      <span className="font-medium">{label}</span>
    </div>
  )
}

interface NavbarProps {
  activePage: string;
  onPageChange: (page: string) => void;
}

const Navbar: React.FC<NavbarProps> = ({ activePage, onPageChange }) => {
  const navItems = [
    { id: 'codex', label: 'Codex', icon: <Terminal size={18} /> },
    { id: 'logithm', label: 'Logithm', icon: <Brain size={18} /> },
    { id: 'vocalith', label: 'Vocalith', icon: <Mic size={18} /> },
    { id: 'logs', label: 'Logs', icon: <FileText size={18} /> },
    { id: 'settings', label: 'Settings', icon: <Settings size={18} /> }
  ]
  
  return (
    <div className="h-16 bg-zinc-900 border-b border-zinc-800 flex items-center px-6 shadow-lg">
      <div className="flex items-center justify-between w-full">
        <div className="flex items-center gap-6">
          {/* Logo */}
          <div className="flex items-center gap-2">
            <div className="w-8 h-8 rounded-md bg-primary/90 flex items-center justify-center">
              <Terminal size={20} className="text-white" />
            </div>
            <span className="text-xl font-semibold text-zinc-100">CodexAI</span>
          </div>
          
          {/* Navigation items */}
          <div className="flex items-center gap-2">
            {navItems.map(item => (
              <NavItem
                key={item.id}
                icon={item.icon}
                label={item.label}
                active={activePage === item.id}
                onClick={() => onPageChange(item.id)}
              />
            ))}
          </div>
        </div>
        
        {/* Right side - could add user profile, etc. */}
        <div>
          {/* Example: Version number */}
          <span className="text-xs text-zinc-500">v1.0.0</span>
        </div>
      </div>
    </div>
  )
}

export default Navbar
