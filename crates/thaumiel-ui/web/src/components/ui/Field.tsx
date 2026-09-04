"use client";

import { InputHTMLAttributes, ReactNode, SelectHTMLAttributes, TextareaHTMLAttributes } from "react";
import styles from "./Field.module.css";

function Wrap({ label, hint, error, children }: { label?: string; hint?: string; error?: string; children: ReactNode }) {
  return (
    <label className={styles.field}>
      {label ? <span className={styles.label}>{label}</span> : null}
      {children}
      {error ? <span className={styles.error}>{error}</span> : hint ? <span className={styles.hint}>{hint}</span> : null}
    </label>
  );
}

interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
  label?: string;
  hint?: string;
  error?: string;
}

export function Input({ label, hint, error, className, ...props }: InputProps) {
  return (
    <Wrap label={label} hint={hint} error={error}>
      <input className={[styles.input, className].filter(Boolean).join(" ")} {...props} />
    </Wrap>
  );
}

interface SelectProps extends SelectHTMLAttributes<HTMLSelectElement> {
  label?: string;
  hint?: string;
}

export function Select({ label, hint, className, children, ...props }: SelectProps) {
  return (
    <Wrap label={label} hint={hint}>
      <select className={[styles.select, className].filter(Boolean).join(" ")} {...props}>
        {children}
      </select>
    </Wrap>
  );
}

interface TextareaProps extends TextareaHTMLAttributes<HTMLTextAreaElement> {
  label?: string;
  hint?: string;
}

export function Textarea({ label, hint, className, ...props }: TextareaProps) {
  return (
    <Wrap label={label} hint={hint}>
      <textarea className={[styles.textarea, className].filter(Boolean).join(" ")} {...props} />
    </Wrap>
  );
}
