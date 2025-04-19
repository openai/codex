import * as React from "react"
import { ChevronDown, ChevronRight, Folder, File } from "lucide-react"
import { cn } from "@/lib/utils"

interface TreeItemProps {
  label: string
  expanded?: boolean
  defaultExpanded?: boolean
  children?: React.ReactNode
  className?: string
  icon?: React.ReactNode
  onClick?: () => void
}

const TreeItem = React.forwardRef<HTMLDivElement, TreeItemProps>(
  ({ 
    label, 
    expanded, 
    defaultExpanded = false,
    children, 
    className,
    icon,
    onClick,
    ...props 
  }, ref) => {
    const [isExpanded, setIsExpanded] = React.useState(defaultExpanded)
    
    const handleToggle = (e: React.MouseEvent) => {
      e.stopPropagation()
      setIsExpanded(!isExpanded)
      onClick?.()
    }
    
    // Use controlled (expanded) or uncontrolled (isExpanded) state
    const showChildren = expanded !== undefined ? expanded : isExpanded
    const hasChildren = !!children
    
    return (
      <div className={cn("select-none", className)} ref={ref} {...props}>
        <div 
          className="flex items-center gap-1 py-1 px-2 rounded hover:bg-primary/10 cursor-pointer"
          onClick={handleToggle}
        >
          {hasChildren ? (
            <div className="text-zinc-400">
              {showChildren ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
            </div>
          ) : <div className="w-4" />}
          
          <div className="mr-1 text-zinc-400">
            {icon || (hasChildren ? <Folder size={16} /> : <File size={16} />)}
          </div>
          
          <div className="text-sm text-zinc-300">{label}</div>
        </div>
        
        {showChildren && hasChildren && (
          <div className="ml-6 pl-2 border-l border-zinc-700/30">
            {children}
          </div>
        )}
      </div>
    )
  }
)
TreeItem.displayName = "TreeItem"

interface TreeViewProps {
  className?: string
  children?: React.ReactNode
}

const TreeView = React.forwardRef<HTMLDivElement, TreeViewProps>(
  ({ className, children, ...props }, ref) => {
    return (
      <div 
        ref={ref} 
        className={cn("py-2", className)} 
        {...props}
      >
        {children}
      </div>
    )
  }
)
TreeView.displayName = "TreeView"

export { TreeView, TreeItem }
