'use client';

import { useEffect, useCallback, useState } from 'react';
import { createPortal } from 'react-dom';

type ConfirmModalProps = {
  isOpen: boolean;
  title: string;
  message: string;
  confirmLabel?: string;
  cancelLabel?: string;
  variant?: 'danger' | 'default';
  confirmText?: string;
  confirmPlaceholder?: string;
  onConfirm: () => void;
  onCancel: () => void;
};

export default function ConfirmModal({
  isOpen,
  title,
  message,
  confirmLabel = 'Confirm',
  cancelLabel = 'Cancel',
  variant = 'default',
  confirmText,
  confirmPlaceholder,
  onConfirm,
  onCancel,
}: ConfirmModalProps) {
  const [inputValue, setInputValue] = useState('');

  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        onCancel();
      }
    },
    [onCancel]
  );

  useEffect(() => {
    if (isOpen) {
      document.addEventListener('keydown', handleKeyDown);
      document.body.style.overflow = 'hidden';
      setInputValue('');
    }
    return () => {
      document.removeEventListener('keydown', handleKeyDown);
      document.body.style.overflow = '';
    };
  }, [isOpen, handleKeyDown]);

  const isConfirmDisabled = confirmText ? inputValue !== confirmText : false;

  if (!isOpen) return null;

  const modalContent = (
    <div
      style={{
        position: 'fixed',
        top: 0,
        left: 0,
        right: 0,
        bottom: 0,
        backgroundColor: 'rgba(0, 0, 0, 0.6)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        zIndex: 1000,
      }}
      onClick={onCancel}
    >
      <div
        style={{
          backgroundColor: 'var(--bg-secondary)',
          borderRadius: 'var(--radius-md)',
          border: '1px solid var(--border-primary)',
          boxShadow: 'var(--shadow-lg)',
          maxWidth: '400px',
          width: '90%',
        }}
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div
          style={{
            display: 'flex',
            justifyContent: 'space-between',
            alignItems: 'center',
            padding: 'var(--spacing-md)',
            borderBottom: '1px solid var(--border-primary)',
          }}
        >
          <h3 style={{ margin: 0, fontSize: '16px', fontWeight: 600 }}>{title}</h3>
          <button
            onClick={onCancel}
            style={{
              background: 'none',
              border: 'none',
              cursor: 'pointer',
              padding: '4px',
              color: 'var(--text-secondary)',
              fontSize: '18px',
              lineHeight: 1,
            }}
          >
            &times;
          </button>
        </div>

        {/* Body */}
        <div
          style={{
            padding: 'var(--spacing-lg) var(--spacing-md)',
            color: 'var(--text-secondary)',
            fontSize: '14px',
            lineHeight: 1.5,
          }}
        >
          {message}
          {confirmText && (
            <div style={{ marginTop: 'var(--spacing-md)' }}>
              <label style={{ display: 'block', marginBottom: 'var(--spacing-xs)', fontSize: '13px' }}>
                Type <strong style={{ color: 'var(--text-primary)' }}>{confirmText}</strong> to confirm:
              </label>
              <input
                type="text"
                value={inputValue}
                onChange={(e) => setInputValue(e.target.value)}
                placeholder={confirmPlaceholder || confirmText}
                style={{ width: '100%', boxSizing: 'border-box' }}
                autoFocus
              />
            </div>
          )}
        </div>

        {/* Footer */}
        <div
          style={{
            display: 'flex',
            justifyContent: 'flex-end',
            gap: 'var(--spacing-sm)',
            padding: 'var(--spacing-md)',
            borderTop: '1px solid var(--border-primary)',
          }}
        >
          <button onClick={onCancel}>{cancelLabel}</button>
          <button
            onClick={onConfirm}
            disabled={isConfirmDisabled}
            className={variant === 'danger' ? '' : 'primary'}
            style={
              variant === 'danger'
                ? {
                    backgroundColor: isConfirmDisabled ? 'var(--text-muted)' : 'var(--accent-red)',
                    color: '#fff',
                    border: 'none',
                    cursor: isConfirmDisabled ? 'not-allowed' : 'pointer',
                  }
                : isConfirmDisabled
                ? { opacity: 0.5, cursor: 'not-allowed' }
                : undefined
            }
          >
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );

  if (typeof window === 'undefined') return null;

  return createPortal(modalContent, document.body);
}
