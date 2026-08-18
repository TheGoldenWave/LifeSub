---
feature_id: [version-需求名称-YYYYMM]
version: 1.0.0
source: [business|product-initiated]
linked_req: [REQ-ID or null]
stage: intake
owner: [产品负责人]
requester: [需求方 or null]
tech_owner: [技术负责人 or TBD]
qa_owner: [测试负责人 or TBD]
last_updated: [YYYY-MM-DD]
---

# 项目进度 — [项目名称]

## 当前状态

- **整体状态**: `intake`
- **风险等级**: 🟢 低
- **预计上线**: [待排期]

## 甘特图

```mermaid
gantt
    title [feature_id] 项目排期
    dateFormat  YYYY-MM-DD
    axisFormat  %m/%d

    section 需求阶段
    需求接收与澄清       :active, intake, [START_DATE], 3d
    需求确认             :confirmation, after intake, 2d
    MRD 编写             :mrd, after confirmation, 1d

    section 设计阶段
    PRD 撰写             :prd, after mrd, 3d
    技术评审             :tech_review, after prd, 2d
    技术方案定稿          :tech_final, after tech_review, 2d

    section 开发阶段
    前端开发             :frontend, after tech_final, 10d
    后端开发             :backend, after tech_final, 12d
    联调测试             :integration, after frontend, 5d

    section 验收阶段
    产品验收             :acceptance, after integration, 3d
    上线发布             :release, after acceptance, 1d
```

## 关键里程碑

| 里程碑 | 计划日期 | 实际日期 | 状态 |
|:---|:---|:---|:---|
| 需求冻结 | [日期] | - | ⏳ 待启动 |
| 开发启动 | [日期] | - | ⏳ 待启动 |
| 提测 | [日期] | - | ⏳ 待启动 |
| 上线 | [日期] | - | ⏳ 待启动 |

## 当前阻塞项

| 问题 | 责任人 | 截止日期 | 影响范围 |
|:---|:---|:---|:---|
| (暂无) | - | - | - |

## 进度日志

### [YYYY-MM-DD]
- 项目初始化，进入需求接收阶段
