import { useIsDark } from "@/hooks/useIsDark";

interface LogoProps {
  className?: string;
  alt?: string;
}

export function Logo({ className, alt = "Aineer" }: LogoProps) {
  const isDark = useIsDark();
  return (
    <img src={isDark ? "/logo-dark.svg" : "/logo-light.svg"} alt={alt} className={className} />
  );
}
