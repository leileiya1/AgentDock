import * as React from "react";
import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/utils";

const buttonVariants = cva(
  "inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-control text-body font-medium transition-[background,border-color,box-shadow,transform] duration-150 ease-[var(--ease-out-expo)] outline-none focus-visible:ring-2 focus-visible:ring-focus/70 disabled:pointer-events-none disabled:opacity-50 active:scale-[0.98] select-none [&_svg]:size-4 [&_svg]:shrink-0",
  {
    variants: {
      variant: {
        default:
          "border border-line bg-raised text-t1 hover:border-line-strong hover:bg-panel",
        primary:
          "border border-action bg-action text-white hover:bg-action-soft",
        human:
          "border border-status-human bg-status-human text-white hover:bg-status-human-soft",
        danger:
          "border border-status-danger/45 text-status-danger hover:border-status-danger/70 hover:bg-status-human-bg/45",
        outline: "border border-line text-t1 hover:bg-raised",
        ghost: "text-t2 hover:bg-raised hover:text-t1",
        subtle: "bg-raised/60 text-t2 hover:bg-raised hover:text-t1",
      },
      size: {
        default: "h-8 px-3",
        sm: "h-7 px-2 text-meta",
        lg: "h-9 px-4 text-sm",
        icon: "size-8",
      },
    },
    defaultVariants: { variant: "default", size: "default" },
  }
);

export interface ButtonProps
  extends React.ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof buttonVariants> {
  asChild?: boolean;
}

export const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className, variant, size, asChild = false, ...props }, ref) => {
    const Comp = asChild ? Slot : "button";
    return (
      <Comp
        ref={ref}
        data-slot="button"
        className={cn(buttonVariants({ variant, size }), className)}
        {...props}
      />
    );
  }
);
Button.displayName = "Button";

export { buttonVariants };
