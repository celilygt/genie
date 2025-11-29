---
name: "explain"
description: "Explain a concept at different complexity levels"
model: "gemini-2.5-pro"
input_variables:
  - name: "topic"
    description: "Topic or concept to explain"
    required: true
  - name: "level"
    description: "Explanation level"
    default: "intermediate"
  - name: "examples"
    description: "Include practical examples"
    default: "yes"
json_output: false
---
Explain "{{ topic }}" at a {{ level }} level.

{% if examples == "yes" %}
Include practical, real-world examples to illustrate the concept.
{% endif %}

Structure your explanation as:
1. **What it is** - A clear definition
2. **Why it matters** - Its importance and use cases
3. **How it works** - The underlying mechanism
{% if examples == "yes" %}
4. **Examples** - Practical illustrations
{% endif %}
5. **Key takeaways** - Main points to remember

Keep the explanation clear and accessible while being technically accurate.

