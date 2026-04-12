"use client";

import { FileText, X } from "lucide-react";
import { useEffect, useRef } from "react";
import type { Translations } from "@/lib/i18n";
import { cn } from "@/lib/utils";
import type { PendingAttachment } from "./types";

function formatSize(bytes: number, u: Translations["units"]): string {
  if (bytes < 1024) return `${bytes} ${u.b}`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} ${u.kb}`;
  return `${(bytes / 1024 / 1024).toFixed(1)} ${u.mb}`;
}

function AttachmentThumb({
  attachment,
  isDragging,
  onRemove,
  onPreview,
  onDragStart,
  onDragEnd,
  onDragOver,
  units,
}: {
  attachment: PendingAttachment;
  isDragging: boolean;
  onRemove: (id: string) => void;
  onPreview: (url: string) => void;
  onDragStart: () => void;
  onDragEnd: () => void;
  onDragOver: (targetId: string) => void;
  units: Translations["units"];
}) {
  const isImage = attachment.type === "image" && attachment.previewUrl;
  const sizeLabel = formatSize(attachment.size, units);

  return (
    <li
      draggable
      className="list-none"
      onDragStart={(e) => {
        e.dataTransfer.effectAllowed = "move";
        onDragStart();
      }}
      onDragEnd={onDragEnd}
      onDragOver={(e) => {
        e.preventDefault();
        e.dataTransfer.dropEffect = "move";
        onDragOver(attachment.id);
      }}
    >
      <div
        className={cn(
          "hover-reveal relative flex-shrink-0 rounded-lg border border-border bg-muted/50 transition-opacity",
          isDragging && "opacity-40",
        )}
      >
        {isImage ? (
          <button
            type="button"
            onClick={() => onPreview(attachment.previewUrl ?? "")}
            className="block overflow-hidden rounded-t-lg"
          >
            <img
              src={attachment.previewUrl}
              alt={attachment.name}
              className="h-16 w-20 object-cover transition-transform hover:scale-105"
            />
          </button>
        ) : (
          <div className="flex h-16 w-20 items-center justify-center rounded-t-lg bg-muted">
            <FileText className="h-6 w-6 text-muted-foreground" />
          </div>
        )}
        <div className="flex flex-col px-1.5 py-1">
          <span className="max-w-[72px] truncate text-[10px] text-foreground">
            {attachment.name}
          </span>
          <span className="text-[9px] text-muted-foreground">{sizeLabel}</span>
        </div>
        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            onRemove(attachment.id);
          }}
          className="hover-reveal-target absolute -top-1.5 -right-1.5 flex h-5 w-5 items-center justify-center rounded-full bg-destructive text-destructive-foreground shadow-sm"
        >
          <X className="h-3 w-3" />
        </button>
      </div>
    </li>
  );
}

export function AttachmentLightbox({
  url,
  alt,
  onClose,
}: {
  url: string;
  alt: string;
  onClose: () => void;
}) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    ref.current?.focus();
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        onClose();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [onClose]);

  return (
    <div
      ref={ref}
      role="dialog"
      tabIndex={-1}
      className="fixed inset-0 z-50 flex cursor-zoom-out items-center justify-center bg-black/70 backdrop-blur-sm"
      onClick={onClose}
      onKeyDown={(e) => {
        if (e.key === "Escape") onClose();
      }}
    >
      {/* biome-ignore lint/a11y/useKeyWithClickEvents: stopPropagation only, no action */}
      <img
        src={url}
        alt={alt}
        className="max-h-[80vh] max-w-[80vw] rounded-lg shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      />
    </div>
  );
}

export function AttachmentStrip({
  attachments,
  dragAttId,
  units,
  onRemove,
  onPreview,
  onDragStart,
  onDragEnd,
  onDragOver,
}: {
  attachments: PendingAttachment[];
  dragAttId: string | null;
  units: Translations["units"];
  onRemove: (id: string) => void;
  onPreview: (url: string) => void;
  onDragStart: (id: string) => void;
  onDragEnd: () => void;
  onDragOver: (fromId: string, toId: string) => void;
}) {
  if (attachments.length === 0) return null;

  return (
    <div className="mb-1.5 flex flex-wrap gap-2">
      {attachments.map((att) => (
        <AttachmentThumb
          key={att.id}
          attachment={att}
          units={units}
          isDragging={dragAttId === att.id}
          onRemove={onRemove}
          onPreview={onPreview}
          onDragStart={() => onDragStart(att.id)}
          onDragEnd={onDragEnd}
          onDragOver={(targetId) => {
            if (dragAttId && dragAttId !== targetId) onDragOver(dragAttId, targetId);
          }}
        />
      ))}
    </div>
  );
}
