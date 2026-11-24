import { useEffect, useRef } from 'react';
import type { FolderRole } from '../types/mail';

interface UseKeyboardNavOptions {
  disabled: boolean;
  onMoveDown: () => void;
  onMoveUp: () => void;
  onArchive: () => void;
  onReply: () => void;
  onCompose: () => void;
  onToggleSelect: () => void;
  onTrash: () => void;
  onToggleRead: () => void;
  onToggleStar: () => void;
  onSnooze: () => void;
  onOpenCommandPalette: () => void;
  onFocusSearch: () => void;
  onToggleShortcuts: () => void;
  onEscape: () => void;
  onJumpFolder: (folder: FolderRole) => void;
}

function isEditableElement(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  const tag = target.tagName.toLowerCase();
  return (
    tag === 'input' ||
    tag === 'textarea' ||
    tag === 'select' ||
    target.isContentEditable
  );
}

/**
 * Superhuman / Linear-grade keyboard-first navigation engine.
 * Supports single-key commands (`j`/`k`, `e`, `r`, `x`, `c`, `u`, `s`, `h`, `#`, `/`, `?`)
 * plus chord sequences (`g` + `i` Inbox, `g` + `v` Vault/Archive, `g` + `s` Sent, `g` + `t` Trash)
 * and global `Cmd+K` / `Ctrl+K` command palette.
 */
export function useKeyboardNav(opts: UseKeyboardNavOptions) {
  const pendingChordRef = useRef<{ key: string; ts: number } | null>(null);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Global Cmd+K / Ctrl+K always opens Command Palette
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') {
        e.preventDefault();
        opts.onOpenCommandPalette();
        return;
      }

      if (e.key === 'Escape') {
        opts.onEscape();
        return;
      }

      if (opts.disabled || isEditableElement(e.target)) {
        return;
      }

      // Ignore system modifier shortcuts except Shift
      if (e.metaKey || e.ctrlKey || e.altKey) {
        return;
      }

      const key = e.key;
      const now = Date.now();

      // Handle `g` + <letter> folder navigation chords (500ms window)
      if (pendingChordRef.current && now - pendingChordRef.current.ts < 600) {
        const chord = pendingChordRef.current.key;
        pendingChordRef.current = null;
        if (chord === 'g') {
          const lower = key.toLowerCase();
          if (lower === 'i') {
            e.preventDefault();
            opts.onJumpFolder('inbox');
            return;
          }
          if (lower === 'v' || lower === 'a') {
            e.preventDefault();
            opts.onJumpFolder('archive');
            return;
          }
          if (lower === 's') {
            e.preventDefault();
            opts.onJumpFolder('sent');
            return;
          }
          if (lower === 'd') {
            e.preventDefault();
            opts.onJumpFolder('drafts');
            return;
          }
          if (lower === 'z') {
            e.preventDefault();
            opts.onJumpFolder('snoozed');
            return;
          }
          if (lower === 't') {
            e.preventDefault();
            opts.onJumpFolder('trash');
            return;
          }
        }
      }

      if (key.toLowerCase() === 'g') {
        pendingChordRef.current = { key: 'g', ts: now };
        return;
      }

      switch (key) {
        case 'j':
        case 'ArrowDown':
          e.preventDefault();
          opts.onMoveDown();
          break;
        case 'k':
        case 'ArrowUp':
          e.preventDefault();
          opts.onMoveUp();
          break;
        case 'e':
          e.preventDefault();
          opts.onArchive();
          break;
        case 'r':
          e.preventDefault();
          opts.onReply();
          break;
        case 'c':
          e.preventDefault();
          opts.onCompose();
          break;
        case 'x':
          e.preventDefault();
          opts.onToggleSelect();
          break;
        case '#':
        case 'Backspace':
          e.preventDefault();
          opts.onTrash();
          break;
        case 'u':
          e.preventDefault();
          opts.onToggleRead();
          break;
        case 's':
          e.preventDefault();
          opts.onToggleStar();
          break;
        case 'h':
          e.preventDefault();
          opts.onSnooze();
          break;
        case '/':
          e.preventDefault();
          opts.onFocusSearch();
          break;
        case '?':
          e.preventDefault();
          opts.onToggleShortcuts();
          break;
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [opts]);
}
