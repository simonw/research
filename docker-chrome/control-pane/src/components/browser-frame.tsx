import React from "react";

interface BrowserFrameProps {
  url: string;
}

export function BrowserFrame({ url }: BrowserFrameProps) {
  return (
    <div className="relative mx-auto border-gray-800 bg-gray-800 border-[14px] rounded-[2.5rem] h-[667px] w-[375px] shadow-xl">
      <div className="w-[148px] h-[18px] bg-gray-800 top-0 rounded-b-[1rem] left-1/2 -translate-x-1/2 absolute z-20"></div>
      <div className="h-[46px] w-[3px] bg-gray-800 absolute -start-[17px] top-[124px] rounded-s-lg"></div>
      <div className="h-[46px] w-[3px] bg-gray-800 absolute -start-[17px] top-[178px] rounded-s-lg"></div>
      <div className="h-[64px] w-[3px] bg-gray-800 absolute -end-[17px] top-[142px] rounded-e-lg"></div>
      <div className="rounded-[2rem] overflow-hidden w-full h-full bg-white relative z-10">
        <iframe
          src={url}
          className="w-full h-full border-none"
          allow="cross-origin-isolated"
          referrerPolicy="no-referrer"
        />
      </div>
    </div>
  );
}
