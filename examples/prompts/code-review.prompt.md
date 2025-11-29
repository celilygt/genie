---
name: "code-review"
description: "Review code for bugs, style issues, and improvements"
model: "gemini-2.5-pro"
input_variables:
  - name: "language"
    description: "Programming language of the code"
    default: "auto-detect"
  - name: "focus"
    description: "Areas to focus on"
    default: "bugs, performance, readability"
  - name: "file"
    type: "file"
    description: "Code file to review"
    required: true
json_output: true
---
You are an expert code reviewer. Analyze the following {{ language }} code and provide a detailed review.

Focus areas: {{ focus }}

Respond with valid JSON in this format:
{
  "summary": "Brief overall assessment",
  "issues": [
    {
      "severity": "high|medium|low",
      "type": "bug|performance|style|security",
      "line": "line number or range",
      "description": "What's wrong",
      "suggestion": "How to fix it"
    }
  ],
  "positives": ["Things done well"],
  "overall_score": 1-10
}

Code to review:
```
{{ file_content }}
```

