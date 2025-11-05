import React, { forwardRef } from 'react';
import {
  Card as MuiCard,
  CardContent,
  CardActions,
  CardHeader,
  CardProps as MuiCardProps,
  SxProps,
  Theme,
} from '@mui/material';
import { motion } from 'framer-motion';

export interface CardProps extends Omit<MuiCardProps, 'sx'> {
  animated?: boolean;
  hover?: boolean;
  header?: React.ReactNode;
  actions?: React.ReactNode;
  sx?: SxProps<Theme>;
}

const MotionCard = motion(MuiCard);

export const Card = forwardRef<HTMLDivElement, CardProps>(
  (
    {
      children,
      animated = false,
      hover = false,
      header,
      actions,
      sx,
      ...props
    },
    ref
  ) => {
    const cardSx: SxProps<Theme> = {
      borderRadius: 3,
      transition: 'all 0.2s cubic-bezier(0.4, 0, 0.2, 1)',
      ...(hover && {
        cursor: 'pointer',
        '&:hover': {
          transform: 'translateY(-2px)',
          boxShadow: '0 8px 25px -5px rgba(0, 0, 0, 0.1), 0 8px 10px -6px rgba(0, 0, 0, 0.1)',
        },
      }),
      ...sx,
    };

    const cardContent = (
      <>
        {header && (
          <CardHeader
            sx={{
              pb: 1,
              '& .MuiCardHeader-title': {
                fontSize: '1.125rem',
                fontWeight: 600,
              },
              '& .MuiCardHeader-subheader': {
                fontSize: '0.875rem',
                color: 'text.secondary',
              },
            }}
            title={typeof header === 'string' ? header : undefined}
          >
            {typeof header !== 'string' && header}
          </CardHeader>
        )}
        <CardContent sx={{ pb: actions ? 1 : 2 }}>
          {children}
        </CardContent>
        {actions && (
          <CardActions sx={{ px: 2, pt: 0, pb: 2 }}>
            {actions}
          </CardActions>
        )}
      </>
    );

    if (animated) {
      return (
        <MotionCard
          ref={ref}
          sx={cardSx}
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: -20 }}
          transition={{
            type: 'spring',
            stiffness: 300,
            damping: 30,
          }}
          {...props}
        >
          {cardContent}
        </MotionCard>
      );
    }

    return (
      <MuiCard ref={ref} sx={cardSx} {...props}>
        {cardContent}
      </MuiCard>
    );
  }
);

Card.displayName = 'Card';

export default Card;
