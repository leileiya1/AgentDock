import * as React from "react";
import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/utils";

const buttonVariants = cva(
  "inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-control text-body font-semibold transition-[background,border-color,box-shadow,transform] duration-100 ease-[var(--ease-out-expo)] outline-none focus-visible:ring-2 focus-visible:ring-focus/70 disabled:pointer-events-none disabled:opacity-50 active:translate-y-0.5 active:shadow-none select-none [&_svg]:size-4 [&_svg]:shrink-0",
  {
    variants: {
      variant: {
        default:
          "border border-line-strong bg-raised text-t1 shadow-press hover:bg-app",
        primary:
          "border border-action bg-action text-white shadow-press-action hover:bg-action-soft",
        human:
          "border border-status-human bg-status-human text-white shadow-press-action hover:bg-status-human-soft",
        danger:
          "border border-status-danger/35 bg-raised text-status-danger shadow-press-danger hover:bg-status-danger-bg",
        outline: "border border-line-strong bg-raised text-t1 shadow-press hover:bg-app",
        ghost: "text-t2 hover:bg-raised hover:text-t1",
        subtle: "bg-raised/60 text-t2 hover:bg-raised hover:text-t1",
      },
      size: {
        default: "h-9 px-3.5",
        sm: "h-8 px-2.5 text-meta",
        lg: "h-10 px-4 text-sm",
        icon: "size-9",
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
