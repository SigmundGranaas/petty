<xsl:stylesheet version="1.0"
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                xmlns:fo="http://www.w3.org/1999/XSL/Format">

    <fo:simple-page-master master-name="A4"
                           page-width="210mm" page-height="297mm"
                           margin="20mm"/>

    <!-- Attribute Sets (must be defined before use) -->
    <xsl:attribute-set name="h1">
        <xsl:attribute name="font-size">24pt</xsl:attribute>
        <xsl:attribute name="font-weight">bold</xsl:attribute>
        <xsl:attribute name="margin-bottom">12pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="h2">
        <xsl:attribute name="font-size">16pt</xsl:attribute>
        <xsl:attribute name="font-weight">bold</xsl:attribute>
        <xsl:attribute name="margin-bottom">4pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="invoice-table">
        <xsl:attribute name="margin-top">20pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="th">
        <xsl:attribute name="font-weight">bold</xsl:attribute>
        <xsl:attribute name="padding">4pt</xsl:attribute>
        <xsl:attribute name="background-color">#EEEEEE</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="th-right" use-attribute-sets="th">
        <xsl:attribute name="text-align">right</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="td">
        <xsl:attribute name="padding">4pt</xsl:attribute>
        <xsl:attribute name="border-bottom">1pt solid #CCCCCC</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="td-right" use-attribute-sets="td">
        <xsl:attribute name="text-align">right</xsl:attribute>
    </xsl:attribute-set>

    <!-- Templates -->
    <xsl:template name="invoice-body">
        <p use-attribute-sets="h1">Invoice <xsl:value-of select="invoiceNumber"/></p>
        <p use-attribute-sets="h2"><xsl:value-of select="name"/></p>
        <p><xsl:value-of select="address"/></p>

        <table use-attribute-sets="invoice-table">
            <columns>
                <column width="50%"/>
                <column width="25%"/>
                <column width="25%"/>
            </columns>
            <header>
                <row>
                    <cell use-attribute-sets="th"><p>Product</p></cell>
                    <cell use-attribute-sets="th-right"><p>Quantity</p></cell>
                    <cell use-attribute-sets="th-right"><p>Price</p></cell>
                </row>
            </header>
            <tbody>
                <xsl:for-each select="items/item">
                    <row>
                        <cell use-attribute-sets="td"><p><xsl:value-of select="product"/></p></cell>
                        <cell use-attribute-sets="td-right"><p><xsl:value-of select="quantity"/></p></cell>
                        <cell use-attribute-sets="td-right"><p><xsl:value-of select="price"/></p></cell>
                    </row>
                </xsl:for-each>
            </tbody>
        </table>
    </xsl:template>

    <xsl:template match="/">
        <!-- Assuming the JSON is {"customers": [...]}, which maps to <customers><item>...</item>... -->
        <xsl:for-each select="customers/item">
            <fo:block>
                <xsl:call-template name="invoice-body"/>
            </fo:block>

            <!-- Add a page break after each invoice except the last one -->
            <xsl:if test="position() != last()">
                <page-break/>
            </xsl:if>
        </xsl:for-each>
    </xsl:template>

</xsl:stylesheet>
