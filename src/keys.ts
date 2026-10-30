export function isTyping(target: EventTarget | null) {
  const el = target as HTMLElement | null;
  return !!el && (el.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(el.tagName));
}

export function arrowNav(event: KeyboardEvent) {
  if (!["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "Home", "End"].includes(event.key)) return;
  if (isTyping(event.target)) return;
  const container = event.currentTarget as HTMLElement;
  const items = [...container.querySelectorAll<HTMLElement>("[data-nav-item]")];
  const from = items.findIndex((el) => el === document.activeElement || el.contains(document.activeElement));
  if (from < 0) return;
  let to = from;
  if (event.key === "Home") to = 0;
  else if (event.key === "End") to = items.length - 1;
  else if (event.key === "ArrowLeft") to = from - 1;
  else if (event.key === "ArrowRight") to = from + 1;
  else {
    const box = items[from].getBoundingClientRect();
    const centre = box.left + box.width / 2;
    const down = event.key === "ArrowDown";
    let best = -1;
    let bestScore = Infinity;
    items.forEach((el, i) => {
      const b = el.getBoundingClientRect();
      const rowGap = down ? b.top - box.top : box.top - b.top;
      if (rowGap < 4) return;
      const score = rowGap * 1000 + Math.abs(b.left + b.width / 2 - centre);
      if (score < bestScore) {
        bestScore = score;
        best = i;
      }
    });
    if (best >= 0) to = best;
  }
  if (to < 0 || to >= items.length || to === from) return;
  event.preventDefault();
  items[to].focus();
  items[to].scrollIntoView({ block: "nearest" });
}
