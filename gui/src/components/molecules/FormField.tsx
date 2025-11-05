import React from 'react';
import {
  Box,
  FormControl,
  FormHelperText,
  FormLabel,
  SxProps,
  Theme,
} from '@mui/material';
import { Input } from '@/components/atoms/Input';
import { LucideIcon } from 'lucide-react';

export interface FormFieldProps {
  label?: string;
  helperText?: string;
  error?: boolean;
  required?: boolean;
  disabled?: boolean;
  startIcon?: LucideIcon;
  endIcon?: LucideIcon;
  placeholder?: string;
  value?: string;
  onChange?: (event: React.ChangeEvent<HTMLInputElement>) => void;
  onBlur?: (event: React.FocusEvent<HTMLInputElement>) => void;
  type?: 'text' | 'email' | 'password' | 'number' | 'tel' | 'url';
  autoComplete?: string;
  sx?: SxProps<Theme>;
}

export const FormField: React.FC<FormFieldProps> = ({
  label,
  helperText,
  error = false,
  required = false,
  disabled = false,
  startIcon,
  endIcon,
  placeholder,
  value,
  onChange,
  onBlur,
  type = 'text',
  autoComplete,
  sx,
}) => {
  return (
    <FormControl
      fullWidth
      error={error}
      disabled={disabled}
      required={required}
      sx={sx}
    >
      {label && (
        <FormLabel
          sx={{
            mb: 1,
            fontWeight: 500,
            fontSize: '0.875rem',
            color: 'text.primary',
            '&.Mui-focused': {
              color: 'primary.main',
            },
            '&.Mui-error': {
              color: 'error.main',
            },
          }}
        >
          {label}
          {required && (
            <Box component="span" sx={{ color: 'error.main', ml: 0.5 }}>
              *
            </Box>
          )}
        </FormLabel>
      )}
      <Input
        type={type}
        placeholder={placeholder}
        value={value}
        onChange={onChange}
        onBlur={onBlur}
        startIcon={startIcon}
        endIcon={endIcon}
        error={error}
        disabled={disabled}
        autoComplete={autoComplete}
      />
      {helperText && (
        <FormHelperText
          sx={{
            mt: 1,
            fontSize: '0.75rem',
            lineHeight: 1.4,
          }}
        >
          {helperText}
        </FormHelperText>
      )}
    </FormControl>
  );
};

export default FormField;
