import { Cpu } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router";

import { CapabilityUnavailablePage } from "@sdkwork/ui-mobile-react";

export function HardwareBind() {
  const { t } = useTranslation("hardware");
  const navigate = useNavigate();

  return (
    <CapabilityUnavailablePage
      icon={Cpu}
      message={t("unavailable")}
      onBack={() => navigate(-1)}
      title={t("title")}
    />
  );
}
