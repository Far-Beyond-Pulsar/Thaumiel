"use client";

import { ButtonHTMLAttributes, forwardRef } from "react";
import styles from "./Button.module.css";

type Variant = "default" | "primary" | "danger" | "ghost";

interface Props extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
  size?: "md" | "sm";
}

const variantClass: Record<Variant, string> = {
  default: styles.btn,
  primary: `${styles.btn} ${styles.primary}`,
  danger: `${styles.btn} ${styles.danger}`,
  ghost: `${styles.btn} ${styles.ghost}`,
};

export const Button = forwardRef<HTMLButtonElement, Props>(function Button(
  { variant = "default", size = "md", className, ...props },
  ref
) {
  const classes = [variantClass[variant], size === "sm" ? styles.sm : "", className].filter(Boolean).join(" ");
  return <button ref={ref} className={classes} {...props} />;
});
