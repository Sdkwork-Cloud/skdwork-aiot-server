import React from "react";
import { cn } from "@sdkwork/ui-mobile-react";
import { Cpu, Speaker, Camera, Bot } from "lucide-react";
import { Hardware } from "../types";
import { useTranslation } from "react-i18next";

interface HardwareCardProps {
  hardware: Hardware;
  onClick: () => void;
  onLongPressProps?: any;
}

export const HardwareCard: React.FC<HardwareCardProps> = ({ hardware: hw, onClick, onLongPressProps }) => {
  const { t } = useTranslation();
const getHardwareIcon = (type: string) => {
  switch (type) {
      case "speaker":
        return <Speaker className="w-6 h-6 text-indigo-500" />;
      case "camera":
        return <Camera className="w-6 h-6 text-emerald-500" />;
      case "robot":
        return <Bot className="w-6 h-6 text-rose-500" />;
      default:
        return <Cpu className="w-6 h-6 text-blue-500" />;
    }
  };

  const bgClassForType = (type: string) => {
  switch (type) {
      case "speaker":
        return "bg-indigo-500/10";
      case "camera":
        return "bg-emerald-500/10";
      case "robot":
        return "bg-rose-500/10";
      default:
        return "bg-blue-500/10";
    }
  };

  return (
    <div
      className="bg-white dark:bg-[#1E1E1E] rounded-2xl p-4 flex flex-col gap-3 shadow-sm border border-black/5 dark:border-white/5 cursor-pointer active:bg-black/5 dark:active:bg-white/5 transition-colors select-none touch-callout-none"
      onClick={onClick}
      {...onLongPressProps}
    >
      <div className="flex gap-3 pointer-events-none">
        <div
          className={cn(
            "w-12 h-12 rounded-xl flex items-center justify-center shrink-0",
            bgClassForType(hw.type)
          )}
        >
          {getHardwareIcon(hw.type)}
        </div>
        <div className="flex-1">
          <div className="flex items-center justify-between mb-1">
            <span className="text-[16px] font-medium text-text-main line-clamp-1 flex-1 pr-2">
              {hw.name}
            </span>
            <div className="flex items-center gap-1.5 shrink-0">
              <span
                className={cn(
                  "w-2 h-2 rounded-full",
                  hw.status === "online" ? "bg-emerald-500" : "bg-gray-400"
                )}
              />
              <span className="text-[12px] text-text-sub">
                {hw.status === "online" ? t('hardware.detail.online') : t('hardware.detail.offline')}
              </span>
            </div>
          </div>
          {hw.agentName ? (
            <div className="flex items-center gap-1.5 mt-1.5 text-[12px]">
              <span className="text-purple-500 flex items-center gap-1 px-1.5 py-0.5 bg-purple-500/10 rounded">
                <Bot className="w-3 h-3" />
                {t('hardware.agentLinked', { agentName: hw.agentName })}
              </span>
            </div>
          ) : (
            <div className="flex items-center gap-1 mt-1 text-[12px] text-text-sub opacity-80">
              <span>{t('hardware.agentUnlinked')}</span>
            </div>
          )}
        </div>
      </div>
    </div>
  );
};

