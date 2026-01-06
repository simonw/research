'use client';

import React, { useState, useEffect, useRef } from 'react';
import { InputSchema } from '@/lib/types';
import { AlertCircle, Loader2 } from 'lucide-react';

interface DynamicFormProps {
  input: InputSchema;
  onSubmit: (values: Record<string, unknown>) => void;
  onCancel: () => void;
  isSubmitting?: boolean;
}

export function DynamicForm({ input, onSubmit, onCancel, isSubmitting }: DynamicFormProps) {
  const { schema, uiSchema = {}, title, description, submitLabel = 'Continue', cancelLabel = 'Cancel', error, errors = {} } = input;
  const [formData, setFormData] = useState<Record<string, any>>({});
  const [localErrors, setLocalErrors] = useState<Record<string, string>>({});
  const firstFieldRef = useRef<HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement>(null);

  useEffect(() => {
    const initialData: Record<string, any> = {};
    Object.entries(schema.properties).forEach(([key, prop]) => {
      if (prop.default !== undefined) {
        initialData[key] = prop.default;
      } else if (prop.type === 'boolean') {
        initialData[key] = false;
      } else {
        initialData[key] = '';
      }
    });
    setFormData(initialData);
  }, [schema]);

  useEffect(() => {
    const timer = setTimeout(() => {
      if (firstFieldRef.current) {
        firstFieldRef.current.focus();
      }
    }, 100);
    return () => clearTimeout(timer);
  }, [input]);

  const handleChange = (key: string, value: any) => {
    setFormData(prev => ({ ...prev, [key]: value }));
    if (localErrors[key]) {
      setLocalErrors(prev => {
        const next = { ...prev };
        delete next[key];
        return next;
      });
    }
  };

  const validate = (): boolean => {
    const newErrors: Record<string, string> = {};
    
    if (schema.required) {
      schema.required.forEach(key => {
        if (!formData[key] && formData[key] !== 0 && formData[key] !== false) {
          newErrors[key] = 'This field is required';
        }
      });
    }

    Object.entries(schema.properties).forEach(([key, prop]) => {
      const value = formData[key];
      if (prop.type === 'string' && value) {
        if (prop.minLength && value.length < prop.minLength) {
          newErrors[key] = `Minimum length is ${prop.minLength} characters`;
        }
        if (prop.maxLength && value.length > prop.maxLength) {
          newErrors[key] = `Maximum length is ${prop.maxLength} characters`;
        }
        if (prop.pattern && !new RegExp(prop.pattern).test(value)) {
          newErrors[key] = 'Invalid format';
        }
      }
    });

    setLocalErrors(newErrors);
    return Object.keys(newErrors).length === 0;
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (isSubmitting) return;
    
    if (validate()) {
      onSubmit(formData);
    }
  };

  const renderField = (key: string, prop: any) => {
    const ui = uiSchema[key] || {};
    const fieldError = localErrors[key] || errors[key];
    const widget = ui['ui:widget'];
    const placeholder = ui['ui:placeholder'] || '';
    const help = ui['ui:help'];
    const isFirstField = Object.keys(schema.properties)[0] === key;
    const autofocus = ui['ui:autofocus'] || isFirstField;

    let inputElement: React.ReactNode;

    if (widget === 'radio' && prop.enum) {
      inputElement = (
        <div className="space-y-3">
          {prop.enum.map((value: string, index: number) => (
            <label key={value} className="flex items-center gap-3 p-4 border border-border rounded-xl cursor-pointer hover:bg-background/50 transition-all active:scale-[0.99] group">
              <input
                type="radio"
                name={key}
                value={value}
                checked={formData[key] === value}
                onChange={() => handleChange(key, value)}
                className="w-5 h-5 text-accent border-border focus:ring-accent transition-all"
              />
              <span className="text-sm font-bold text-foreground group-hover:text-accent transition-colors">
                {prop.enumNames ? prop.enumNames[index] : value}
              </span>
            </label>
          ))}
        </div>
      );
    } else if (widget === 'select' && prop.enum) {
      inputElement = (
        <div className="relative">
          <select
            ref={autofocus ? (firstFieldRef as any) : null}
            value={formData[key]}
            onChange={(e) => handleChange(key, e.target.value)}
            className="w-full px-4 py-3.5 bg-background border border-border rounded-xl text-foreground focus:ring-2 focus:ring-accent/20 focus:border-accent outline-none appearance-none transition-all shadow-sm"
          >
            {prop.enum.map((value: string, index: number) => (
              <option key={value} value={value}>
                {prop.enumNames ? prop.enumNames[index] : value}
              </option>
            ))}
          </select>
          <div className="absolute right-4 top-1/2 -translate-y-1/2 pointer-events-none text-text-tertiary">
            <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M3 4.5L6 7.5L9 4.5"/></svg>
          </div>
        </div>
      );
    } else if (prop.type === 'boolean') {
      inputElement = (
        <label className="flex items-center gap-3 cursor-pointer">
          <input
            type="checkbox"
            checked={!!formData[key]}
            onChange={(e) => handleChange(key, e.target.checked)}
            className="w-4 h-4 text-accent border-border rounded focus:ring-accent"
          />
          <span className="text-sm text-text-secondary">{prop.title}</span>
        </label>
      );
    } else if (widget === 'textarea') {
      inputElement = (
        <textarea
          ref={autofocus ? (firstFieldRef as any) : null}
          value={formData[key]}
          onChange={(e) => handleChange(key, e.target.value)}
          placeholder={placeholder}
          rows={3}
          className="w-full px-4 py-2.5 bg-background border border-border rounded-lg text-foreground focus:ring-2 focus:ring-accent/20 focus:border-accent outline-none transition-all resize-none"
        />
      );
    } else {
      inputElement = (
        <input
          ref={autofocus ? (firstFieldRef as any) : null}
          type={widget === 'password' ? 'password' : 'text'}
          value={formData[key]}
          onChange={(e) => handleChange(key, e.target.value)}
          placeholder={placeholder}
          className="w-full px-4 py-3.5 bg-background border border-border rounded-xl text-foreground focus:ring-2 focus:ring-accent/20 focus:border-accent outline-none transition-all shadow-sm"
        />
      );
    }

    return (
      <div key={key} className="space-y-1.5">
        {prop.type !== 'boolean' && widget !== 'radio' && (
          <label className="block text-xs font-bold text-text-tertiary uppercase tracking-widest ml-1">
            {prop.title}
          </label>
        )}
        {inputElement}
        {help && <p className="text-xs text-text-tertiary ml-1">{help}</p>}
        {fieldError && (
          <p className="text-xs text-error font-bold flex items-center gap-1 mt-1 ml-1">
            <AlertCircle size={14} />
            {fieldError}
          </p>
        )}
      </div>
    );
  };

  return (
    <form onSubmit={handleSubmit} className="space-y-8">
      {title && <h2 className="text-xl font-bold text-foreground tracking-tight">{title}</h2>}
      {description && <p className="text-sm text-text-secondary leading-relaxed">{description}</p>}

      {error && (
        <div className="p-4 bg-error/10 border border-error/20 rounded-2xl flex items-start gap-3 shadow-sm animate-in shake-1">
          <AlertCircle className="text-error mt-0.5 shrink-0" size={18} />
          <p className="text-sm text-error font-bold">{error}</p>
        </div>
      )}

      <div className="space-y-6">
        {Object.entries(schema.properties).map(([key, prop]) => renderField(key, prop))}
      </div>

      <div className="pt-4 flex flex-col gap-4">
        <button
          type="submit"
          disabled={isSubmitting}
          className="w-full py-4 bg-foreground text-background font-bold rounded-xl shadow-lg hover:opacity-95 disabled:opacity-50 transition-all flex items-center justify-center gap-2 active:scale-[0.98]"
        >
          {isSubmitting ? (
            <>
              <Loader2 className="animate-spin" size={20} />
              Linking...
            </>
          ) : (
            submitLabel
          )}
        </button>
        <button
          type="button"
          onClick={onCancel}
          disabled={isSubmitting}
          className="w-full py-2 text-sm font-bold text-text-tertiary hover:text-foreground transition-colors"
        >
          {cancelLabel}
        </button>
      </div>
    </form>
  );
}
