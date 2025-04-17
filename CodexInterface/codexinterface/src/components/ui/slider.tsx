import * as React from "react"
import { cn } from "../../lib/utils"

interface SliderProps {
  className?: string;
  min?: number;
  max?: number;
  step?: number;
  value: number[];
  onValueChange?: (value: number[]) => void;
  disabled?: boolean;
  id?: string;
}

const Slider = React.forwardRef<HTMLDivElement, SliderProps>(
  ({ className, min = 0, max = 100, step = 1, value, onValueChange, disabled, id, ...props }, ref) => {
    const trackRef = React.useRef<HTMLDivElement>(null)
    const thumbRef = React.useRef<HTMLDivElement>(null)
    const [isDragging, setIsDragging] = React.useState(false)
    const [displayValue, setDisplayValue] = React.useState(value[0])

    // Calculate percentage for positioning the thumb
    const percentage = ((value[0] - min) / (max - min)) * 100
    
    // Handle mouse down on the thumb
    const handleMouseDown = (event: React.MouseEvent) => {
      if (disabled) return
      event.preventDefault()
      setIsDragging(true)
      document.addEventListener("mousemove", handleMouseMove)
      document.addEventListener("mouseup", handleMouseUp)
    }
    
    // Handle mouse move
    const handleMouseMove = (event: MouseEvent) => {
      if (!trackRef.current) return
      
      const rect = trackRef.current.getBoundingClientRect()
      const x = Math.max(0, Math.min(event.clientX - rect.left, rect.width))
      const newPercentage = (x / rect.width) * 100
      const newValue = Math.round((newPercentage / 100) * (max - min) / step) * step + min
      
      // Ensure value stays within min and max
      const clampedValue = Math.max(min, Math.min(max, newValue))
      setDisplayValue(clampedValue)
      
      if (onValueChange) {
        onValueChange([clampedValue])
      }
    }
    
    // Handle mouse up
    const handleMouseUp = () => {
      setIsDragging(false)
      document.removeEventListener("mousemove", handleMouseMove)
      document.removeEventListener("mouseup", handleMouseUp)
    }
    
    // Clean up event listeners
    React.useEffect(() => {
      return () => {
        document.removeEventListener("mousemove", handleMouseMove)
        document.removeEventListener("mouseup", handleMouseUp)
      }
    }, [])
    
    return (
      <div
        ref={ref}
        id={id}
        className={cn(
          "relative flex w-full touch-none select-none items-center",
          disabled && "opacity-50 cursor-not-allowed",
          className
        )}
        {...props}
      >
        <div
          ref={trackRef}
          className="relative h-2 w-full grow overflow-hidden rounded-full bg-zinc-700/50"
        >
          <div
            className="absolute h-full bg-primary/80"
            style={{ width: `${percentage}%` }}
          />
        </div>
        <div
          ref={thumbRef}
          className={cn(
            "absolute h-4 w-4 rounded-full border-2 border-primary bg-background ring-offset-background transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:pointer-events-none disabled:opacity-50",
            isDragging && "cursor-grabbing",
            !isDragging && "cursor-grab"
          )}
          style={{
            left: `calc(${percentage}% - 0.5rem)`,
          }}
          onMouseDown={handleMouseDown}
          tabIndex={disabled ? -1 : 0}
        />
      </div>
    )
  }
)

Slider.displayName = "Slider"

export { Slider }
