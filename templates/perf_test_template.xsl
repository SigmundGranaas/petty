<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet version="1.0"
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                xmlns:petty="https://docs.rs/petty">

    <!-- 1. Page Layout Definition -->
    <petty:page-layout size="Letter"
                       margin="40pt 40pt 40pt 40pt"
                       footer-height="30pt"
                       footer-text="Report ID: {{id}} | Page {{page_num}} of {{total_pages}}"
                       footer-style="footer"/>

    <!-- 2. Style Definitions (Attribute Sets) -->
    <xsl:attribute-set name="pageTitle">
        <xsl:attribute name="font-size">18pt</xsl:attribute>
        <xsl:attribute name="font-weight">bold</xsl:attribute>
        <xsl:attribute name="color">#142850</xsl:attribute>
        <xsl:attribute name="margin-bottom">20pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="headerBox">
        <xsl:attribute name="padding">15pt</xsl:attribute>
        <xsl:attribute name="background-color">#F0F5FA</xsl:attribute>
        <xsl:attribute name="border">1pt solid #D2DCE6</xsl:attribute>
        <xsl:attribute name="margin-bottom">25pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="userName">
        <xsl:attribute name="font-size">14pt</xsl:attribute>
        <xsl:attribute name="font-weight">bold</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="accountInfo">
        <xsl:attribute name="font-size">11pt</xsl:attribute>
        <xsl:attribute name="color">#646464</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="hr">
        <xsl:attribute name="height">1pt</xsl:attribute>
        <xsl:attribute name="background-color">#C8C8C8</xsl:attribute>
        <xsl:attribute name="margin-top">25pt</xsl:attribute>
        <xsl:attribute name="margin-bottom">15pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="th">
        <xsl:attribute name="font-weight">bold</xsl:attribute>
        <xsl:attribute name="text-align">left</xsl:attribute>
        <xsl:attribute name="color">#FFFFFF</xsl:attribute>
        <xsl:attribute name="background-color">#465569</xsl:attribute>
        <xsl:attribute name="padding">8pt 6pt 8pt 6pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="td">
        <xsl:attribute name="font-size">10pt</xsl:attribute>
        <xsl:attribute name="padding">6pt</xsl:attribute>
        <xsl:attribute name="border">0.5pt solid #DCDCDC</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="td-currency">
        <xsl:attribute name="font-size">10pt</xsl:attribute>
        <xsl:attribute name="text-align">right</xsl:attribute>
        <xsl:attribute name="padding">6pt</xsl:attribute>
        <xsl:attribute name="border">0.5pt solid #DCDCDC</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="summaryContainer">
        <xsl:attribute name="margin-top">20pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="summaryLabel">
        <xsl:attribute name="font-size">11pt</xsl:attribute>
        <xsl:attribute name="text-align">right</xsl:attribute>
        <xsl:attribute name="margin-bottom">4pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="summaryValue">
        <xsl:attribute name="font-size">12pt</xsl:attribute>
        <xsl:attribute name="font-weight">bold</xsl:attribute>
        <xsl:attribute name="text-align">right</xsl:attribute>
        <xsl:attribute name="margin-bottom">4pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="footer">
        <xsl:attribute name="font-size">9pt</xsl:attribute>
        <xsl:attribute name="color">#808080</xsl:attribute>
        <xsl:attribute name="text-align">center</xsl:attribute>
    </xsl:attribute-set>

    <!-- 3. Main Document Template -->
    <xsl:template match="/">
        <document>
            <!-- Create a new page for each record in the data -->
            <page-sequence select="/records">
                <text style="pageTitle">Transaction Summary</text>
                <container style="headerBox">
                    <text style="userName">{{user.name}}</text>
                    <text style="accountInfo">Account: {{user.account}}</text>
                </container>

                <text style="userName">Details:</text>
                <rectangle style="hr"/>

                <table>
                    <columns>
                        <column width="50%"/>
                        <column width="15%" style="td-currency" header-style="th"/>
                        <column width="15%" style="td-currency" header-style="th"/>
                        <column width="20%" style="td-currency" header-style="th"/>
                    </columns>
                    <header>
                        <row>
                            <cell style="th">Item Description</cell>
                            <cell style="th">Qty</cell>
                            <cell style="th">Unit Price</cell>
                            <cell style="th">Total</cell>
                        </row>
                    </header>
                    <tbody>
                        <xsl:for-each select="items">
                            <row>
                                <cell style="td"><xsl:value-of select="description"/></cell>
                                <cell style="td-currency"><xsl:value-of select="quantity"/></cell>
                                <cell style="td-currency">{{formatCurrency price}}</cell>
                                <cell style="td-currency">{{formatCurrency line_total}}</cell>
                            </row>
                        </xsl:for-each>
                    </tbody>
                </table>
                <container style="summaryContainer">
                    <text style="summaryLabel">Subtotal: {{formatCurrency summary.total}}</text>
                    <text style="summaryLabel">Tax (8%): {{formatCurrency summary.tax}}</text>
                    <text style="summaryValue">Grand Total: {{formatCurrency summary.grand_total}}</text>
                </container>
            </page-sequence>
        </document>
    </xsl:template>
</xsl:stylesheet>