import { useStdout } from "ink"
import { useEffect, useState } from "react"

type TerminalSize = { rows: number; columns: number }

export const useTerminalSize = (): TerminalSize => {
  const { stdout } = useStdout()
  const [size, setSize] = useState<TerminalSize>({
    rows: stdout.rows ?? 24,
    columns: stdout.columns ?? 80,
  })

  useEffect(() => {
    const onResize = () =>
      setSize({ rows: stdout.rows ?? 24, columns: stdout.columns ?? 80 })
    stdout.on("resize", onResize)
    return () => {
      stdout.off("resize", onResize)
    }
  }, [stdout])

  return size
}
