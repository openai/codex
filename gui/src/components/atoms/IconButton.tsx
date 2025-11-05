import React, { forwardRef } from 'react';
import {
  IconButton as MuiIconButton,
  IconButtonProps as MuiIconButtonProps,
  Tooltip,
  SxProps,
  Theme,
} from '@mui/material';
import { motion } from 'framer-motion';
import { LucideIcon } from 'lucide-react';

export interface IconButtonProps extends Omit<MuiIconButtonProps, 'sx'> {
  icon: LucideIcon;
  tooltip?: string;
  animated?: boolean;
  variant?: 'contained' | 'outlined' | 'standard';
  size?: 'small' | 'medium' | 'large';
  sx?: SxProps<Theme>;
}

const MotionIconButton = motion(MuiIconButton);

export const IconButton = forwardRef<HTMLButtonElement, IconButtonProps>(
  (
    {
      icon: Icon,
      tooltip,
      animated = true,
      variant = 'standard',
      size = 'medium',
      sx,
      children,
      ...props
    },
    ref
  ) => {
    const iconSize = size === 'small' ? 16 : size === 'large' ? 24 : 20;

    const iconButtonSx: SxProps<Theme> = {
      borderRadius: 2,
      transition: 'all 0.2s cubic-bezier(0.4, 0, 0.2, 1)',
      '&:focus-visible': {
        outline: '2px solid',
        outlineOffset: '2px',
        outlineColor: 'primary.main',
      },
      ...(variant === 'contained' && {
        backgroundColor: 'primary.main',
        color: 'primary.contrastText',
        boxShadow: '0 1px 3px 0 rgba(0, 0, 0, 0.12), 0 1px 1px 0 rgba(0, 0, 0, 0.08)',
        '&:hover': {
          backgroundColor: 'primary.dark',
          boxShadow: '0 2px 8px 0 rgba(0, 0, 0, 0.15), 0 2px 4px 0 rgba(0, 0, 0, 0.12)',
        },
      }),
      ...(variant === 'outlined' && {
        border: '1px solid',
        borderColor: 'outline.main',
        '&:hover': {
          backgroundColor: 'action.hover',
          borderColor: 'primary.main',
        },
      }),
      ...sx,
    };

    const buttonElement = animated ? (
      <MotionIconButton
        ref={ref}
        size={size}
        sx={iconButtonSx}
        whileHover={{ scale: 1.05 }}
        whileTap={{ scale: 0.95 }}
        transition={{
          type: 'spring',
          stiffness: 400,
          damping: 17,
        }}
        {...props}
      >
        <Icon size={iconSize} />
        {children}
      </MotionIconButton>
    ) : (
      <MuiIconButton
        ref={ref}
        size={size}
        sx={iconButtonSx}
        {...props}
      >
        <Icon size={iconSize} />
        {children}
      </MuiIconButton>
    );

    if (tooltip) {
      return (
        <Tooltip
          title={tooltip}
          placement="top"
          enterDelay={300}
          leaveDelay={200}
          arrow
        >
          {buttonElement}
        </Tooltip>
      );
    }

    return buttonElement;
  }
);

IconButton.displayName = 'IconButton';

export default IconButton;
