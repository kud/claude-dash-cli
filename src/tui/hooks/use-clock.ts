import { useEffect, useState } from "react"

const padTwo = (n: number) => String(n).padStart(2, "0")

const formatTime = (d: Date) =>
  `${padTwo(d.getHours())}:${padTwo(d.getMinutes())}:${padTwo(d.getSeconds())}`

export const useClock = (): string => {
  const [time, setTime] = useState(() => formatTime(new Date()))

  useEffect(() => {
    const timer = setInterval(() => setTime(formatTime(new Date())), 1000)
    return () => clearInterval(timer)
  }, [])

  return time
}
