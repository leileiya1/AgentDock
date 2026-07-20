import * as React from "react";
import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/utils";

const buttonVariants = cva(
  "inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-[var(--radius-control)] text-[13px] font-medium transition-[background,border-color,box-shadow,transform] duration-150 ease-[var(--ease-out-expo)] outline-none focus-visible:ring-2 focus-visible:ring-run/70 disabled:pointer-events-none disabled:opacity-50 active:scale-[0.98] select-none [&_svg]:size-4 [&_svg]:shrink-0",
  {
    variants: {
      variant: {
        default:
          "border border-line bg-raised/80 text-t1 hover:border-line-strong hover:bg-[#222b36]",
        primary:
          "border border-run bg-gradient-to-b from-run-soft to-run text-white shadow-[0_1px_0_rgba(255,255,255,0.22)_inset,var(--shadow-glow-run)] hover:brightness-105",
        human:
          "border border-human bg-gradient-to-b from-human-soft to-human text-white shadow-[0_1px_0_rgba(255,255,255,0.25)_inset,var(--shadow-glow-human)] hover:brightness-105",
        danger:
          "border border-[#3a2a2d] text-bad hover:bg-[#2a1c1e] hover:border-bad/60",
        outline: "border border-line text-t1 hover:bg-raised",
        ghost: "text-t2 hover:bg-raised hover:text-t1",
        subtle: "bg-raised/60 text-t2 hover:bg-raised hover:text-t1",
      },
      size: {
        default: "h-8 px-3",
        sm: "h-7 px-2 text-[12px]",
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
