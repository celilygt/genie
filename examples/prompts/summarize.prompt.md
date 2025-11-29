---
name: "summarize"
description: "Summarize text content in various styles"
model: "gemini-2.5-pro"
input_variables:
  - name: "style"
    description: "Summary style"
    default: "concise"
  - name: "max_points"
    description: "Maximum number of key points"
    default: "5"
  - name: "language"
    description: "Output language"
    default: "en"
  - name: "file"
    type: "file"
    description: "File to summarize"
    required: true
json_output: false
---
Summarize the following content in a {{ style }} style.
Output language: {{ language }}
Maximum key points: {{ max_points }}

Content to summarize:
{{ file_content }}

Provide:
1. A one-paragraph summary
2. Up to {{ max_points }} key points as bullet points
3. Any important terms or concepts mentioned

