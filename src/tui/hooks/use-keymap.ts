import { useInput } from "ink"

type KeymapOptions = {
  onUp: () => void
  onDown: () => void
  onAllow: () => void
  onDeny: () => void
  onQuit: () => void
  onQuitAndKill: () => void
}

export const useKeymap = ({
  onUp,
  onDown,
  onAllow,
  onDeny,
  onQuit,
  onQuitAndKill,
}: KeymapOptions): void => {
  useInput((input, key) => {
    if (key.upArrow || input === "k") return onUp()
    if (key.downArrow || input === "j") return onDown()
    if (input === "a") return onAllow()
    if (input === "d") return onDeny()
    if (input === "q") return onQuit()
    if (input === "Q" || (key.ctrl && input === "q")) return onQuitAndKill()
  })
}
