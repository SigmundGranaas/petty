<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet version="1.0"
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                xmlns:fo="http://www.w3.org/1999/XSL/Format">

    <!-- Page Layout Definition -->
    <fo:layout-master-set>
        <fo:simple-page-master master-name="report-page"
                               page-width="612pt"
                               page-height="792pt"
                               margin-top="40pt"
                               margin-bottom="40pt"
                               margin-left="40pt"
                               margin-right="40pt"
                               footer-text="Report ID: {{id}} | Page {{page_num}} of {{total_pages}}"
                               footer-style="footer"/>
    </fo:layout-master-set>

    <!-- Style Definitions -->
    <xsl:attribute-set name="pageTitle">
        <xsl:attribute name="font-size">18pt</xsl:attribute>
        <xsl:attribute name="font-weight">bold</xsl:attribute>
        <xsl:attribute name="color">#142850</xsl:attribute>
        <xsl:attribute name="margin-bottom">20pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="headerBox">
        <xsl:attribute name="padding">15pt</xsl:attribute>
        <xsl:attribute name="background-color">#F0F5FA</xsl:attribute>
        <xsl:attribute name="border">1pt solid #D2DCe6</xsl:attribute>
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
        <xsl:attribute name="margin">25pt 0pt 15pt 0pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="th">
        <xsl:attribute name="font-weight">bold</xsl:attribute>
        <xsl:attribute name="text-align">left</xsl:attribute>
        <xsl:attribute name="color">#FFFFFF</xsl:attribute>
        <xsl:attribute name="background-color">#465569</xsl:attribute>
        <xsl:attribute name="padding">8pt 6pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="td">
        <xsl:attribute name="padding">6pt</xsl:attribute>
        <xsl:attribute name="font-size">10pt</xsl:attribute>
        <xsl:attribute name="border">0.5pt solid #DCDCDC</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="td-currency">
        <xsl:attribute name="padding">6pt</xsl:attribute>
        <xsl:attribute name="font-size">10pt</xsl:attribute>
        <xsl:attribute name="text-align">right</xsl:attribute>
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

    <!-- Root Template -->
    <xsl:template match="/">
        <document>
            <!-- The page-sequence creates a new page FOR EACH record -->
            <page-sequence select="/records">
                <text style="pageTitle">Transaction Summary</text>

                <container style="headerBox">
                    <text style="userName"><xsl:value-of select="user/name"/></text>
                    <text style="accountInfo">Account: <xsl:value-of select="user/account"/></text>
                </container>

                <text style="userName">Details:</text>
                <rectangle style="hr"/>

                <table>
                    <columns>
                        <column width="55%" header-style="th" style="td"/>
                        <column width="15%" header-style="th" style="td-currency"/>
                        <column width="15%" header-style="th" style="td-currency"/>
                        <column width="15%" header-style="th" style="td-currency"/>
                    </columns>
                    <header>
                        <row>
                            <cell>Item Description</cell>
                            <cell>Qty</cell>
                            <cell>Unit Price</cell>
                            <cell>Total</cell>
                        </row>
                    </header>
                    <tbody>
                        <xsl:for-each select="items">
                            <row>
                                <cell><xsl:value-of select="description"/></cell>
                                <cell><xsl:value-of select="quantity"/></cell>
                                <cell>{{formatCurrency price}}</cell>
                                <cell>{{formatCurrency line_total}}</cell>
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