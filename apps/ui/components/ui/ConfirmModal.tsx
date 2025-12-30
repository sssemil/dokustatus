'use client';

import { useState, useEffect } from 'react';
import { AlertTriangle } from 'lucide-react';
import { Modal } from './Modal';
import { Button } from './Button';
import { Input } from './Input';
import { HoldButton } from './HoldButton';

interface ConfirmModalProps {
  isOpen: boolean;
  title: string;
  message: string;
  confirmLabel?: string;
  cancelLabel?: string;
  variant?: 'danger' | 'default';
  confirmText?: string;
  confirmPlaceholder?: string;
  useHoldToConfirm?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

export function ConfirmModal({
  isOpen,
  title,
  message,
  confirmLabel = 'Confirm',
  cancelLabel = 'Cancel',
  variant = 'default',
  confirmText,
  confirmPlaceholder,
  useHoldToConfirm = false,
  onConfirm,
  onCancel,
}: ConfirmModalProps) {
  const [inputValue, setInputValue] = useState('');

  // Reset input when modal opens/closes
  useEffect(() => {
    if (isOpen) {
      setInputValue('');
    }
  }, [isOpen]);

  const isConfirmDisabled = confirmText ? inputValue !== confirmText : false;

  const handleConfirm = () => {
    if (!isConfirmDisabled) {
      onConfirm();
    }
  };

  return (
    <Modal open={isOpen} onClose={onCancel} title={title} size="sm">
      <div className="space-y-4">
        {/* Warning icon for danger variant */}
        {variant === 'danger' && (
          <div className="flex items-center gap-3 p-3 bg-red-500/10 border border-red-500/20 rounded-lg">
            <AlertTriangle size={20} className="text-red-400 flex-shrink-0" />
            <p className="text-sm text-red-200">{message}</p>
          </div>
        )}

        {variant !== 'danger' && (
          <p className="text-sm text-zinc-400">{message}</p>
        )}

        {/* Type to confirm input */}
        {confirmText && (
          <div className="space-y-2">
            <label className="block text-sm text-zinc-400">
              Type <span className="font-medium text-white">{confirmText}</span> to confirm:
            </label>
            <Input
              value={inputValue}
              onChange={(e) => setInputValue(e.target.value)}
              placeholder={confirmPlaceholder || confirmText}
              autoFocus
            />
          </div>
        )}

        {/* Actions */}
        <div className="flex justify-end gap-2 pt-2">
          <Button variant="ghost" onClick={onCancel}>
            {cancelLabel}
          </Button>
          {useHoldToConfirm && variant === 'danger' ? (
            <HoldButton
              onComplete={handleConfirm}
              disabled={isConfirmDisabled}
              variant="danger"
              duration={3000}
            >
              {confirmLabel}
            </HoldButton>
          ) : (
            <Button
              variant={variant === 'danger' ? 'danger' : 'primary'}
              onClick={handleConfirm}
              disabled={isConfirmDisabled}
            >
              {confirmLabel}
            </Button>
          )}
        </div>
      </div>
    </Modal>
  );
}
