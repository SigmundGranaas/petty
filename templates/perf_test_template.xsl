<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet version="1.0"
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                xmlns:fo="http://www.w3.org/1999/XSL/Format">

    <!-- 1. PAGE LAYOUT & FOOTER -->
    <fo:simple-page-master
            page-width="8.5in"
            page-height="11in"
            margin-top="50pt"
            margin-bottom="50pt"
            margin-left="50pt"
            margin-right="50pt"
            footer-text="Report ID: {{id}} | Page {{page_num}}"
            footer-style="footer"
    />

    <!-- 2. STYLE DEFINITIONS (Attribute Sets) -->

    <!-- Typography & Spacing Styles -->
    <xsl:attribute-set name="page-title">
        <xsl:attribute name="font-size">28pt</xsl:attribute>
        <xsl:attribute name="font-weight">bold</xsl:attribute>
        <xsl:attribute name="color">#0D47A1</xsl:attribute>
        <xsl:attribute name="margin-bottom">25pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="account-info">
        <xsl:attribute name="font-size">11pt</xsl:attribute>
        <xsl:attribute name="color">#424242</xsl:attribute>
        <xsl:attribute name="margin-bottom">35pt</xsl:attribute>
        <xsl:attribute name="padding">8pt 12pt</xsl:attribute>
        <xsl:attribute name="background-color">#F5F5F5</xsl:attribute> <!-- Light gray background -->
    </xsl:attribute-set>

    <xsl:attribute-set name="section-title">
        <xsl:attribute name="font-size">18pt</xsl:attribute>
        <xsl:attribute name="font-weight">normal</xsl:attribute>
        <xsl:attribute name="color">#0D47A1</xsl:attribute>
        <xsl:attribute name="margin-bottom">15pt</xsl:attribute>
        <xsl:attribute name="padding-bottom">5pt</xsl:attribute>
        <xsl:attribute name="border-bottom">1pt solid #B0BEC5</xsl:attribute> <!-- A subtle blue-gray line -->
    </xsl:attribute-set>

    <!-- Table Styles -->
    <xsl:attribute-set name="th">
        <xsl:attribute name="font-weight">bold</xsl:attribute>
        <xsl:attribute name="text-align">left</xsl:attribute>
        <xsl:attribute name="padding">6pt 6pt</xsl:attribute>
        <xsl:attribute name="border-bottom">2pt solid #0D47A1</xsl:attribute>
        <xsl:attribute name="color">#37474F</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="th-right">
        <xsl:attribute name="font-weight">bold</xsl:attribute>
        <xsl:attribute name="text-align">right</xsl:attribute>
        <xsl:attribute name="padding">6pt 6pt</xsl:attribute>
        <xsl:attribute name="border-bottom">2pt solid #0D47A1</xsl:attribute>
        <xsl:attribute name="color">#37474F</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="td">
        <xsl:attribute name="padding">5pt 6pt</xsl:attribute>
        <xsl:attribute name="font-size">10pt</xsl:attribute>
        <xsl:attribute name="border-bottom">1pt solid #ECEFF1</xsl:attribute>
        <xsl:attribute name="color">#263238</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="td-right">
        <xsl:attribute name="padding">5pt 6pt</xsl:attribute>
        <xsl:attribute name="font-size">10pt</xsl:attribute>
        <xsl:attribute name="text-align">right</xsl:attribute>
        <xsl:attribute name="border-bottom">1pt solid #ECEFF1</xsl:attribute>
        <xsl:attribute name="color">#263238</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="summary-container">
        <xsl:attribute name="margin-top">5pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="summary-label">
        <xsl:attribute name="text-align">right</xsl:attribute>
        <xsl:attribute name="padding">2pt 10pt</xsl:attribute>
        <xsl:attribute name="font-size">11pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="summary-value">
        <xsl:attribute name="text-align">right</xsl:attribute>
        <xsl:attribute name="padding">4pt 6pt</xsl:attribute>
        <xsl:attribute name="font-size">11pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="summary-total-label">
        <xsl:attribute name="text-align">right</xsl:attribute>
        <xsl:attribute name="padding">2pt 10pt 2pt 6pt</xsl:attribute>
        <xsl:attribute name="font-size">14pt</xsl:attribute>
        <xsl:attribute name="font-weight">bold</xsl:attribute>
        <xsl:attribute name="border-top">1.5pt solid #263238</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="summary-total-value">
        <xsl:attribute name="text-align">right</xsl:attribute>
        <xsl:attribute name="padding">2pt 6pt 2pt 6pt</xsl:attribute>
        <xsl:attribute name="font-size">14pt</xsl:attribute>
        <xsl:attribute name="font-weight">bold</xsl:attribute>
        <xsl:attribute name="border-top">1.5pt solid #263238</xsl:attribute>
    </xsl:attribute-set>

    <!-- Footer Style -->
    <xsl:attribute-set name="footer">
        <xsl:attribute name="font-size">9pt</xsl:attribute>
        <xsl:attribute name="color">#78909C</xsl:attribute>
        <xsl:attribute name="text-align">center</xsl:attribute>
    </xsl:attribute-set>


    <!-- 3. DOCUMENT STRUCTURE TEMPLATE -->
    <xsl:template match="/">
        <page-sequence select="records">
            <text style="page-title">Transaction Summary</text>

            <text style="account-info">
                Account: <xsl:value-of select="user/account"/>
            </text>

            <text style="section-title">Details</text>

            <table>
                <columns>
                    <column width="45%"/>
                    <column width="15%"/>
                    <column width="20%"/>
                    <column width="20%"/>
                </columns>
                <header>
                    <row>
                        <cell style="th"><text>Item</text></cell>
                        <cell style="th-right"><text>Qty</text></cell>
                        <cell style="th-right"><text>Unit Price</text></cell>
                        <cell style="th-right"><text>Total</text></cell>
                    </row>
                </header>
                <tbody>
                    <xsl:for-each select="items">
                        <row>
                            <cell style="td"><text><xsl:value-of select="description"/></text></cell>
                            <cell style="td-right"><text><xsl:value-of select="quantity"/></text></cell>
                            <cell style="td-right">
                                <text>{{formatCurrency price}}</text>
                            </cell>
                            <cell style="td-right">
                                <text>{{formatCurrency line_total}}</text>
                            </cell>
                        </row>
                    </xsl:for-each>
                </tbody>
            </table>

            <flex-container>
                <container width="60%"/>
                <container width="40%" style="summary-container">
                    <table>
                        <columns>
                            <column width="auto"/>
                            <column width="auto"/>
                        </columns>
                        <tbody>
                            <row>
                                <cell style="summary-label"><text>Subtotal:</text></cell>
                                <cell style="summary-value"><text>{{formatCurrency summary/total}}</text></cell>
                            </row>
                            <row>
                                <cell style="summary-label"><text>Tax (8%):</text></cell>
                                <cell style="summary-value"><text>{{formatCurrency summary/tax}}</text></cell>
                            </row>
                            <row>
                                <cell style="summary-total-label"><text>Grand Total:</text></cell>
                                <cell style="summary-total-value"><text>{{formatCurrency summary/grand_total}}</text></cell>
                            </row>
                        </tbody>
                    </table>
                </container>
            </flex-container>
        </page-sequence>
    </xsl:template>
</xsl:stylesheet>