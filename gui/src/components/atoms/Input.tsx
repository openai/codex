import React, { forwardRef } from 'react';
import {
  TextField,
  TextFieldProps,
  InputAdornment,
  SxProps,
  Theme,
} from '@mui/material';
import { LucideIcon } from 'lucide-react';

export interface InputProps extends Omit<TextFieldProps, 'sx'> {
  startIcon?: LucideIcon;
  endIcon?: LucideIcon;
  helperText?: string;
  error?: boolean;
  sx?: SxProps<Theme>;
}

export const Input = forwardRef<HTMLInputElement, InputProps>(
  (
    {
      startIcon: StartIcon,
      endIcon: EndIcon,
      helperText,
      error = false,
      sx,
      variant = 'outlined',
      size = 'medium',
      fullWidth = true,
      ...props
    },
    ref
  ) => {
    const inputSx: SxProps<Theme> = {
      '& .MuiOutlinedInput-root': {
        borderRadius: 3,
        transition: 'all 0.2s cubic-bezier(0.4, 0, 0.2, 1)',
        '&:hover .MuiOutlinedInput-notchedOutline': {
          borderColor: error ? 'error.main' : 'primary.main',
          borderWidth: '2px',
        },
        '&.Mui-focused .MuiOutlinedInput-notchedOutline': {
          borderColor: error ? 'error.main' : 'primary.main',
          borderWidth: '2px',
        },
        '&.Mui-focused': {
          boxShadow: error
            ? '0 0 0 3px rgba(186, 26, 26, 0.12)'
            : '0 0 0 3px rgba(0, 97, 164, 0.12)',
        },
      },
      '& .MuiInputLabel-root': {
        '&.Mui-focused': {
          color: error ? 'error.main' : 'primary.main',
        },
      },
      ...sx,
    };

    const InputProps = {
      startAdornment: StartIcon ? (
        <InputAdornment position="start">
          <StartIcon size={20} />
        </InputAdornment>
      ) : undefined,
      endAdornment: EndIcon ? (
        <InputAdornment position="end">
          <EndIcon size={20} />
        </InputAdornment>
      ) : undefined,
    };

    return (
      <TextField
        ref={ref}
        variant={variant}
        size={size}
        fullWidth={fullWidth}
        error={error}
        helperText={helperText}
        sx={inputSx}
        InputProps={InputProps}
        {...props}
      />
    );
  }
);

Input.displayName = 'Input';

export default Input;
