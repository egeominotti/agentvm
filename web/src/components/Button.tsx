import type { ButtonHTMLAttributes } from "react";

type Props = ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: "default" | "primary" | "ghost" | "danger";
  size?: "md" | "sm";
};

export function Button({ variant = "default", size = "md", className = "", type = "button", ...rest }: Props) {
  return <button type={type} className={`btn btn-${variant} btn-${size} ${className}`.trim()} {...rest} />;
}

export function IconButton({ className = "", type = "button", ...rest }: ButtonHTMLAttributes<HTMLButtonElement>) {
  return <button type={type} className={`icon-btn ${className}`.trim()} {...rest} />;
}
