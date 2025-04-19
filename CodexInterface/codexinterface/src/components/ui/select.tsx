import * as React from "react"
import { cn } from "../../lib/utils"
import { ChevronDown } from "lucide-react"

// Since we're seeing issues with Radix UI dependencies, we'll implement a simpler select
// This is a basic custom implementation that mimics the look of Radix UI select

interface SelectProps {
  value?: string;
  onValueChange?: (value: string) => void;
  children: React.ReactNode;
}

const Select: React.FC<SelectProps> = ({ value, onValueChange, children }) => {
  const [open, setOpen] = React.useState(false)
  const wrapperRef = React.useRef<HTMLDivElement>(null)
  
  // Handle clicks outside to close dropdown
  React.useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (wrapperRef.current && !wrapperRef.current.contains(event.target as Node)) {
        setOpen(false)
      }
    }
    
    document.addEventListener('mousedown', handleClickOutside)
    return () => {
      document.removeEventListener('mousedown', handleClickOutside)
    }
  }, [wrapperRef])
  
  return (
    <div ref={wrapperRef} className="relative">
      {React.Children.map(children, child => {
        if (React.isValidElement(child) && child.type === SelectTrigger) {
          return React.cloneElement(child as React.ReactElement<any>, {
            onClick: () => setOpen(!open),
            open
          })
        }
        if (React.isValidElement(child) && child.type === SelectContent) {
          return open ? React.cloneElement(child as React.ReactElement<any>, {
            value,
            onValueChange: (val: string) => {
              if (onValueChange) onValueChange(val)
              setOpen(false)
            }
          }) : null
        }
        return child
      })}
    </div>
  )
}

interface SelectTriggerProps {
  className?: string;
  children: React.ReactNode;
  open?: boolean;
  onClick?: () => void;
}

const SelectTrigger: React.FC<SelectTriggerProps> = ({ 
  className, 
  children, 
  open,
  onClick
}) => {
  return (
    <div
      className={cn(
        "flex h-10 w-full items-center justify-between rounded-md border border-zinc-700 bg-zinc-800/40 px-3 py-2 text-sm text-zinc-200 ring-offset-background cursor-pointer focus:outline-none focus:ring-2 focus:ring-primary/30 focus:ring-offset-2 focus:ring-offset-zinc-900",
        className
      )}
      onClick={onClick}
    >
      {children}
      <ChevronDown className={`h-4 w-4 transition-transform ${open ? 'rotate-180' : ''}`} />
    </div>
  )
}

interface SelectContentProps {
  className?: string;
  children: React.ReactNode;
  value?: string;
  onValueChange?: (value: string) => void;
}

const SelectContent: React.FC<SelectContentProps> = ({ 
  className, 
  children,
  value,
  onValueChange
}) => {
  return (
    <div
      className={cn(
        "absolute z-50 min-w-[8rem] w-full overflow-hidden rounded-md border border-zinc-700 bg-zinc-800 text-zinc-200 shadow-md mt-1",
        className
      )}
    >
      <div className="w-full p-1">
        {React.Children.map(children, child => {
          if (React.isValidElement(child) && child.type === SelectItem) {
            return React.cloneElement(child as React.ReactElement<any>, {
              onSelect: onValueChange,
              isSelected: value === child.props.value
            })
          }
          return child
        })}
      </div>
    </div>
  )
}

interface SelectItemProps {
  className?: string;
  children: React.ReactNode;
  value: string;
  onSelect?: (value: string) => void;
  isSelected?: boolean;
}

const SelectItem: React.FC<SelectItemProps> = ({ 
  className, 
  children, 
  value,
  onSelect,
  isSelected
}) => {
  return (
    <div
      className={cn(
        "relative flex w-full cursor-pointer select-none items-center rounded-sm py-1.5 px-2 text-sm outline-none hover:bg-zinc-700 hover:text-zinc-100",
        isSelected ? "bg-zinc-700 text-zinc-100" : "",
        className
      )}
      onClick={() => onSelect && onSelect(value)}
    >
      {children}
    </div>
  )
}

const SelectValue: React.FC<{ placeholder?: string; children?: React.ReactNode }> = ({ 
  placeholder,
  children
}) => {
  return (
    <span className="text-sm">
      {children || placeholder}
    </span>
  )
}

export { Select, SelectTrigger, SelectContent, SelectItem, SelectValue }
