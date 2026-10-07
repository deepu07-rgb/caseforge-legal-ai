import { useCallback, useEffect, useRef, useState } from "react";

export type MicrophoneState =
  | "UNAVAILABLE"
  | "IDLE"
  | "REQUESTING_PERMISSION"
  | "RECORDING"
  | "PROCESSING"
  | "DENIED"
  | "ERROR";

export interface CapturedAudio {
  audio_base64: string;
  audio_format: string;
  mime_type: string;
  bytes: number;
  duration_ms: number;
}

const supportedMimeType = (): string => {
  const candidates = ["audio/webm;codecs=opus", "audio/webm", "audio/ogg;codecs=opus"];
  return candidates.find((candidate) => MediaRecorder.isTypeSupported(candidate)) ?? "";
};

const blobToBase64 = (blob: Blob): Promise<string> =>
  new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () => reject(reader.error ?? new Error("Unable to read microphone audio"));
    reader.onloadend = () => resolve(String(reader.result).split(",", 2)[1] ?? "");
    reader.readAsDataURL(blob);
  });

export function useMicrophoneCapture() {
  const available =
    typeof navigator !== "undefined" &&
    Boolean(navigator.mediaDevices?.getUserMedia) &&
    typeof MediaRecorder !== "undefined";
  const [state, setState] = useState<MicrophoneState>(available ? "IDLE" : "UNAVAILABLE");
  const [error, setError] = useState<string | null>(null);
  const recorderRef = useRef<MediaRecorder | null>(null);
  const streamRef = useRef<MediaStream | null>(null);
  const chunksRef = useRef<Blob[]>([]);
  const startedAtRef = useRef(0);

  const release = useCallback(() => {
    streamRef.current?.getTracks().forEach((track) => track.stop());
    streamRef.current = null;
    recorderRef.current = null;
    chunksRef.current = [];
  }, []);

  const cancel = useCallback(() => {
    const recorder = recorderRef.current;
    if (recorder && recorder.state !== "inactive") {
      recorder.onstop = null;
      recorder.stop();
    }
    release();
    setError(null);
    setState(available ? "IDLE" : "UNAVAILABLE");
  }, [available, release]);

  useEffect(() => cancel, [cancel]);

  const start = useCallback(async () => {
    if (!available) {
      setState("UNAVAILABLE");
      return false;
    }
    setError(null);
    setState("REQUESTING_PERMISSION");
    try {
      const stream = await navigator.mediaDevices.getUserMedia({
        audio: { echoCancellation: true, noiseSuppression: true, autoGainControl: true },
        video: false,
      });
      const mimeType = supportedMimeType();
      const recorder = mimeType ? new MediaRecorder(stream, { mimeType }) : new MediaRecorder(stream);
      streamRef.current = stream;
      recorderRef.current = recorder;
      chunksRef.current = [];
      recorder.ondataavailable = (event) => {
        if (event.data.size) chunksRef.current.push(event.data);
      };
      recorder.onerror = () => {
        setError("Microphone recording failed.");
        setState("ERROR");
        release();
      };
      startedAtRef.current = Date.now();
      recorder.start(250);
      setState("RECORDING");
      return true;
    } catch (reason) {
      const denied = reason instanceof DOMException && reason.name === "NotAllowedError";
      setError(
        denied
          ? "Microphone permission was denied. Allow microphone access or continue with text."
          : reason instanceof Error
            ? reason.message
            : String(reason),
      );
      setState(denied ? "DENIED" : "ERROR");
      release();
      return false;
    }
  }, [available, release]);

  const stop = useCallback(async (): Promise<CapturedAudio> => {
    const recorder = recorderRef.current;
    if (!recorder || recorder.state === "inactive") {
      throw new Error("No microphone recording is active.");
    }
    setState("PROCESSING");
    return new Promise((resolve, reject) => {
      recorder.onerror = () => {
        release();
        setState("ERROR");
        reject(new Error("Microphone recording failed."));
      };
      recorder.onstop = () => {
        void (async () => {
          try {
            const mimeType = recorder.mimeType || "audio/webm";
            const blob = new Blob(chunksRef.current, { type: mimeType });
            const audioBase64 = await blobToBase64(blob);
            const format = mimeType.includes("ogg") ? "ogg" : "webm";
            resolve({
              audio_base64: audioBase64,
              audio_format: format,
              mime_type: mimeType,
              bytes: blob.size,
              duration_ms: Date.now() - startedAtRef.current,
            });
            setState("IDLE");
          } catch (reason) {
            setState("ERROR");
            reject(reason);
          } finally {
            release();
          }
        })();
      };
      recorder.stop();
    });
  }, [release]);

  return { available, state, error, start, stop, cancel };
}
