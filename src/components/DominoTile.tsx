import React from 'react';
import { motion } from 'motion/react';

interface DominoTileProps {
  value: [number, number];
  horizontal?: boolean;
  className?: string;
  onClick?: () => void;
  isDouble?: boolean;
  isPlayable?: boolean;
  disabled?: boolean;
  scale?: number;
}

const DotPattern = ({ count, horizontal, scale = 1 }: { count: number, horizontal: boolean, scale?: number }) => {
  if (count === 0) return <div className="w-full h-full" />;

  const dotSize = Math.max(1.5, (window.innerWidth >= 768 ? 6 : window.innerWidth >= 640 ? 4 : 2.5) * scale);
  const dotClass = "bg-[#1a1207] rounded-full";
  const dotStyle = { width: `${dotSize}px`, height: `${dotSize}px` };

  if (count === 1) return <div className="w-full h-full flex items-center justify-center"><div className={dotClass} style={dotStyle} /></div>;
  if (count === 2) return <div className={`w-full h-full flex justify-between p-0.5 sm:p-1 ${horizontal ? 'items-center' : 'flex-col'}`}><div className={`self-start ${dotClass}`} style={dotStyle} /><div className={`self-end ${dotClass}`} style={dotStyle} /></div>;
  if (count === 3) return <div className="w-full h-full flex flex-col justify-between p-0.5 sm:p-1"><div className={`self-start ${dotClass}`} style={dotStyle} /><div className={`self-center ${dotClass}`} style={dotStyle} /><div className={`self-end ${dotClass}`} style={dotStyle} /></div>;
  if (count === 4) return (
    <div className="w-full h-full flex flex-col justify-between p-0.5 sm:p-1">
      <div className="flex justify-between"><div className={dotClass} style={dotStyle} /><div className={dotClass} style={dotStyle} /></div>
      <div className="flex justify-between"><div className={dotClass} style={dotStyle} /><div className={dotClass} style={dotStyle} /></div>
    </div>
  );
  if (count === 5) return (
    <div className="w-full h-full flex flex-col justify-between p-0.5 sm:p-1 relative">
      <div className="flex justify-between"><div className={dotClass} style={dotStyle} /><div className={dotClass} style={dotStyle} /></div>
      <div className="absolute inset-0 flex items-center justify-center"><div className={dotClass} style={dotStyle} /></div>
      <div className="flex justify-between"><div className={dotClass} style={dotStyle} /><div className={dotClass} style={dotStyle} /></div>
    </div>
  );
  if (count === 6) return (
    <div className={`w-full h-full flex justify-between p-0.5 sm:p-1 ${horizontal ? 'flex-col' : 'flex-row'}`}>
      <div className={`flex justify-between ${horizontal ? 'w-full' : 'flex-col h-full'}`}><div className={dotClass} style={dotStyle} /><div className={dotClass} style={dotStyle} /><div className={dotClass} style={dotStyle} /></div>
      <div className={`flex justify-between ${horizontal ? 'w-full' : 'flex-col h-full'}`}><div className={dotClass} style={dotStyle} /><div className={dotClass} style={dotStyle} /><div className={dotClass} style={dotStyle} /></div>
    </div>
  );
  return null;
};

export const DominoTile: React.FC<DominoTileProps> = ({
  value,
  horizontal = false,
  className = '',
  onClick,
  isDouble = false,
  isPlayable = false,
  disabled = false,
  scale = 1
}) => {

  const getBaseWidth = () => {
    if (typeof window === 'undefined') return horizontal ? 32 : 16;
    const w = window.innerWidth;
    if (w >= 768) return horizontal ? 72 : 36;
    if (w >= 640) return horizontal ? 56 : 28;
    return horizontal ? 32 : 16;
  };

  const getBaseHeight = () => {
    if (typeof window === 'undefined') return horizontal ? 16 : 32;
    const w = window.innerWidth;
    if (w >= 768) return horizontal ? 36 : 72;
    if (w >= 640) return horizontal ? 28 : 56;
    return horizontal ? 16 : 32;
  };

  const width = getBaseWidth() * scale;
  const height = getBaseHeight() * scale;

  return (
    <motion.div
      initial={{ opacity: 0, scale: 0.8 }}
      animate={{ opacity: 1, scale: 1 }}
      exit={{ opacity: 0, scale: 0.8 }}
      onClick={!disabled ? onClick : undefined}
      style={{ width: `${width}px`, height: `${height}px` }}
      className={`bg-[#f5f0e1] rounded-md shadow-[0_2px_4px_rgba(0,0,0,0.4)] border flex items-center justify-center overflow-hidden shrink-0 transition-all
        ${horizontal ? 'flex-row' : 'flex-col'} 
        ${!disabled && (onClick || isPlayable) ? 'cursor-pointer hover:-translate-y-1 hover:shadow-[0_4px_8px_rgba(0,0,0,0.5)]' : ''}
        ${isPlayable && !disabled ? 'border-amber-400 shadow-[0_0_8px_rgba(245,158,11,0.5)]' : 'border-[#c9b99a]'}
        ${disabled ? 'opacity-20 brightness-50 cursor-not-allowed' : ''}
        ${className}
      `}
    >
      <div className={`flex-1 flex items-center justify-center ${horizontal ? 'border-r border-[#c9b99a]/60' : 'border-b border-[#c9b99a]/60'} w-full h-full`}>
        <DotPattern count={value[0]} horizontal={horizontal} scale={scale} />
      </div>
      <div className="flex-1 flex items-center justify-center w-full h-full">
        <DotPattern count={value[1]} horizontal={horizontal} scale={scale} />
      </div>
    </motion.div>
  );
};
