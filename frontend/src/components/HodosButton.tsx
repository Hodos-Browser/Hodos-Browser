import React from 'react';
import styles from './HodosButton.module.css';

const cx = (...args: (string | false | undefined)[]) => args.filter(Boolean).join(' ');

export interface HodosButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant: 'primary' | 'secondary' | 'danger' | 'ghost' | 'icon';
  size?: 'default' | 'small';
  loading?: boolean;
  loadingText?: string;
}

export const HodosButton = React.forwardRef<HTMLButtonElement, HodosButtonProps>(
  ({ variant, size = 'default', loading, loadingText, disabled, className, children, ...rest }, ref) => {
    return (
      <button
        ref={ref}
        className={cx(
          styles.btn,
          styles[variant],
          styles[size],
          loading && styles.loading,
          className
        )}
        disabled={disabled || loading}
        {...rest}
      >
        {loading && <span className={styles.spinner} />}
        {loading && loadingText ? loadingText : children}
      </button>
    );
  }
);

HodosButton.displayName = 'HodosButton';
