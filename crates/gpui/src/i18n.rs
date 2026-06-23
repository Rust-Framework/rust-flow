//! 国际化模块：支持中英文切换。
//!
//! [`Language`] 枚举表示当前语言，通过 [`FlowEditorView::set_language`](crate::editor::FlowEditorView::set_language)
//! 切换。所有 UI 文字（节点类型标签、面板标题、按钮提示等）通过
//! [`t`] 函数根据当前语言获取对应文案。
//!
//! 节点 label 不受 i18n 影响（由 `node.data["label"]` 决定），
//! i18n 仅影响框架 UI 文字（类型标签、按钮 tooltip、模式名称等）。

/// 语言枚举。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Language {
    /// 中文
    #[default]
    Zh,
    /// 英文
    En,
}

impl Language {
    /// 切换语言。
    pub fn toggle(self) -> Self {
        match self {
            Self::Zh => Self::En,
            Self::En => Self::Zh,
        }
    }

    /// 是否为中文。
    pub fn is_zh(self) -> bool {
        matches!(self, Self::Zh)
    }
}

/// 翻译键：框架 UI 中所有需要国际化的文字。
///
/// 节点 label 不在此列（由 node.data 决定）。
#[derive(Clone, Copy, Debug)]
pub enum TKey {
    // 节点类型标签
    Start,
    End,
    Action,
    Condition,
    Loop,
    // Condition 节点
    If,
    Else,
    // Loop 模式
    LoopForEach,
    LoopForLoop,
    LoopWhile,
    LoopParallel,
    // 面板
    PanelTitle,
    PanelKind,
    PanelLabel,
    PanelDesc,
    PanelNodeName,
    PanelConditions,
    PanelAddBranch,
    PanelLoopMode,
    PanelLoopExpr,
    PanelElseHint,
    PanelConditionTitle,
    PanelLoopTitle,
    PanelNodeSuffix,
    // 按钮提示
    BtnDelete,
    BtnToggleCollapse,
    BtnToggleExpand,
    BtnCollapseBody,
    BtnExpandBody,
    // 收起指示
    Collapsed,
    // Condition 收起态提示
    ConditionsCount,
}

/// 根据语言和翻译键返回对应文案。
pub fn t(lang: Language, key: TKey) -> &'static str {
    match lang {
        Language::Zh => t_zh(key),
        Language::En => t_en(key),
    }
}

fn t_zh(key: TKey) -> &'static str {
    match key {
        TKey::Start => "开始",
        TKey::End => "结束",
        TKey::Action => "动作",
        TKey::Condition => "条件",
        TKey::Loop => "循环",
        TKey::If => "如果",
        TKey::Else => "否则",
        TKey::LoopForEach => "遍历 item",
        TKey::LoopForLoop => "for i in 0..n",
        TKey::LoopWhile => "while cond",
        TKey::LoopParallel => "并行 each",
        TKey::PanelTitle => "属性",
        TKey::PanelKind => "类型",
        TKey::PanelLabel => "标签",
        TKey::PanelDesc => "描述",
        TKey::PanelNodeName => "节点名称",
        TKey::PanelConditions => "条件分支",
        TKey::PanelAddBranch => "+ 添加分支",
        TKey::PanelLoopMode => "循环模式",
        TKey::PanelLoopExpr => "条件表达式 (rhai)",
        TKey::PanelElseHint => "Else 为兜底分支，无需配置条件",
        TKey::PanelConditionTitle => "条件节点（条件分支）",
        TKey::PanelLoopTitle => "循环节点（循环）",
        TKey::PanelNodeSuffix => "节点",
        TKey::BtnDelete => "删除",
        TKey::BtnToggleCollapse => "收起",
        TKey::BtnToggleExpand => "展开",
        TKey::BtnCollapseBody => "收起循环体",
        TKey::BtnExpandBody => "展开循环体",
        TKey::Collapsed => "已收起",
        TKey::ConditionsCount => "个条件",
    }
}

fn t_en(key: TKey) -> &'static str {
    match key {
        TKey::Start => "Start",
        TKey::End => "End",
        TKey::Action => "Action",
        TKey::Condition => "Condition",
        TKey::Loop => "Loop",
        TKey::If => "If",
        TKey::Else => "Else",
        TKey::LoopForEach => "For each item",
        TKey::LoopForLoop => "for i in 0..n",
        TKey::LoopWhile => "while cond",
        TKey::LoopParallel => "parallel each",
        TKey::PanelTitle => "Properties",
        TKey::PanelKind => "Kind",
        TKey::PanelLabel => "Label",
        TKey::PanelDesc => "Description",
        TKey::PanelNodeName => "Node Name",
        TKey::PanelConditions => "Conditions",
        TKey::PanelAddBranch => "+ Add Branch",
        TKey::PanelLoopMode => "Loop Mode",
        TKey::PanelLoopExpr => "Condition Expression (rhai)",
        TKey::PanelElseHint => "Else is the fallback branch",
        TKey::PanelConditionTitle => "Condition Node",
        TKey::PanelLoopTitle => "Loop Node",
        TKey::PanelNodeSuffix => "Node",
        TKey::BtnDelete => "Delete",
        TKey::BtnToggleCollapse => "Collapse",
        TKey::BtnToggleExpand => "Expand",
        TKey::BtnCollapseBody => "Collapse Body",
        TKey::BtnExpandBody => "Expand Body",
        TKey::Collapsed => "Collapsed",
        TKey::ConditionsCount => "conditions",
    }
}
