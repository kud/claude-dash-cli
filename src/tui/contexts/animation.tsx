import React, {
  createContext,
  useContext,
  useState,
  useEffect,
  memo,
} from "react"

const DOTS = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"] as const
const ARC = ["◐", "◓", "◑", "◒"] as const

export const AnimationContext = createContext(0)

export const useAnimationFrame = () => useContext(AnimationContext)

export const spinnerChar = (type: "dots" | "arc", frame: number): string =>
  type === "dots"
    ? (DOTS[frame % DOTS.length] ?? "⠋")
    : (ARC[frame % ARC.length] ?? "◐")

type AnimationProviderProps = {
  active: boolean
  children: React.ReactNode
}

export const AnimationProvider = memo(
  ({ active, children }: AnimationProviderProps) => {
    const [frame, setFrame] = useState(0)

    useEffect(() => {
      if (!active) return
      const timer = setInterval(() => setFrame((f) => (f + 1) % 20), 250)
      return () => clearInterval(timer)
    }, [active])

    return (
      <AnimationContext.Provider value={frame}>
        {children}
      </AnimationContext.Provider>
    )
  },
)
