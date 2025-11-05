import React, { forwardRef } from 'react';
import {
  Button as MuiButton,
  ButtonProps as MuiButtonProps,
  CircularProgress,
  SxProps,
  Theme,
} from '@mui/material';
import { motion } from 'framer-motion';

export interface ButtonProps extends Omit<MuiButtonProps, 'sx'> {
  loading?: boolean;
  loadingText?: string;
  fullWidth?: boolean;
  animated?: boolean;
  variant?: 'contained' | 'outlined' | 'text';
  size?: 'small' | 'medium' | 'large';
  color?: 'primary' | 'secondary' | 'error' | 'warning' | 'info' | 'success';
  sx?: SxProps<Theme>;
}

const MotionButton = motion(MuiButton);

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  (
    {
      children,
      loading = false,
      loadingText,
      disabled,
      fullWidth = false,
      animated = true,
      variant = 'contained',
      size = 'medium',
      color = 'primary',
      sx,
      onClick,
      ...props
    },
    ref
  ) => {
    const handleClick = (event: React.MouseEvent<HTMLButtonElement>) => {
      if (loading || disabled) return;
      onClick?.(event);
    };

    const buttonContent = (
      <>
        {loading && (
          <CircularProgress
            size={size === 'small' ? 16 : size === 'large' ? 24 : 20}
            sx={{
              mr: 1,
              color: variant === 'contained' ? 'inherit' : 'currentColor',
            }}
          />
        )}
        {loading ? loadingText || children : children}
      </>
    );

    const buttonSx: SxProps<Theme> = {
      minWidth: size === 'small' ? 64 : size === 'large' ? 120 : 80,
      fontWeight: 500,
      letterSpacing: '0.02em',
      transition: 'all 0.2s cubic-bezier(0.4, 0, 0.2, 1)',
      '&:focus-visible': {
        outline: '2px solid',
        outlineOffset: '2px',
        outlineColor: 'primary.main',
      },
      ...sx,
    };

    if (animated) {
      return (
        <MotionButton
          ref={ref}
          variant={variant}
          size={size}
          color={color}
          disabled={loading || disabled}
          fullWidth={fullWidth}
          sx={buttonSx}
          onClick={handleClick}
          whileHover={{ scale: disabled ? 1 : 1.02 }}
          whileTap={{ scale: disabled ? 1 : 0.98 }}
          transition={{
            type: 'spring',
            stiffness: 400,
            damping: 17,
          }}
          {...props}
        >
          {buttonContent}
        </MotionButton>
      );
    }

    return (
      <MuiButton
        ref={ref}
        variant={variant}
        size={size}
        color={color}
        disabled={loading || disabled}
        fullWidth={fullWidth}
        sx={buttonSx}
        onClick={handleClick}
        {...props}
      >
        {buttonContent}
      </MuiButton>
    );
  }
);

Button.displayName = 'Button';

export default Button;
