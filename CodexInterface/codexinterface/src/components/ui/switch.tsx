import * as React from "react"
import { cn } from "../../lib/utils"

export interface SwitchProps extends React.InputHTMLAttributes<HTMLInputElement> {
  onCheckedChange?: (checked: boolean) => void;
}

const Switch = React.forwardRef<HTMLInputElement, SwitchProps>(
  ({ className, onCheckedChange, checked, ...props }, ref) => {
    const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
      if (onCheckedChange) {
        onCheckedChange(e.target.checked);
      }
      if (props.onChange) {
        props.onChange(e);
      }
    };

    return (
      <div className="relative inline-flex items-center cursor-pointer">
        <input 
          type="checkbox" 
          ref={ref}
          className="sr-only" 
          checked={checked}
          onChange={handleChange}
          {...props} 
        />
        <div className={cn(
          "relative w-11 h-6 bg-zinc-700 peer-focus:outline-none rounded-full peer peer-checked:after:translate-x-full after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-zinc-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-primary/80",
          className
        )}></div>
      </div>
    )
  }
)
Switch.displayName = "Switch"

export { Switch }
