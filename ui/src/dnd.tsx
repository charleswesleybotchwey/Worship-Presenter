import { createContext, useContext } from "react";
import type { ProgramItem } from "./api";
import { ContextMenu } from "./features/service/panes";

// Dragging library items (songs, scripture, media, presentations) to the schedule,
// and the right-click menu every library item shares.

export const DRAG_TYPE = "application/x-wp-item";

/** Drags one item, or several (a multiple selection), to the schedule. */
export function dragItem(e: React.DragEvent, what: ProgramItem | ProgramItem[]) {
  const items = Array.isArray(what) ? what : [what];
  e.dataTransfer.setData(DRAG_TYPE, JSON.stringify(items.length === 1 ? items[0] : items));
  const item = items[0];
  e.dataTransfer.setData("text/plain", items.length > 1 ? `${items.length} items` : "title" in item ? String(item.title) : item.kind === "scripture" ? item.reference : "item");
  e.dataTransfer.effectAllowed = "copy";
}

/** Sends a library item straight to the screens (set by the service window). */
/** Go live with an item (start: the slide label to begin at, e.g. a verse "3:16"). */
export const GoLiveContext = createContext<(item: ProgramItem, start?: string) => void>(() => {});
export const useGoLive = () => useContext(GoLiveContext);

/** Preview / Go live / Add to schedule, plus anything item-specific (Edit…). */
export function ItemMenu({
  x,
  y,
  item,
  onClose,
  onPreview,
  onAdd,
  children,
}: {
  x: number;
  y: number;
  item: ProgramItem;
  onClose: () => void;
  onPreview: (i: ProgramItem) => void;
  onAdd: (i: ProgramItem) => void;
  children?: React.ReactNode;
}) {
  const live = useGoLive();
  return (
    <ContextMenu x={x} y={y} onClose={onClose}>
      <button role="menuitem" onClick={() => (onClose(), onPreview(item))}>
        Preview
      </button>
      <button role="menuitem" onClick={() => (onClose(), live(item))}>
        Go live
      </button>
      <button role="menuitem" onClick={() => (onClose(), onAdd(item))}>
        Add to schedule
      </button>
      {children}
    </ContextMenu>
  );
}
