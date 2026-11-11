import { createContext, useContext } from "react";
import type { ProgramItem } from "./api";
import { ContextMenu } from "./features/service/panes";

// Dragging library items (songs, scripture, media, presentations) to the schedule,
// and the right-click menu every library item shares.

export const DRAG_TYPE = "application/x-wp-item";

export function dragItem(e: React.DragEvent, item: ProgramItem) {
  e.dataTransfer.setData(DRAG_TYPE, JSON.stringify(item));
  e.dataTransfer.setData("text/plain", "title" in item ? String(item.title) : item.kind === "scripture" ? item.reference : "item");
  e.dataTransfer.effectAllowed = "copy";
}

/** Sends a library item straight to the screens (set by the service window). */
export const GoLiveContext = createContext<(item: ProgramItem) => void>(() => {});
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
